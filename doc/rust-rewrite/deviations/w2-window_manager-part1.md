# Deviations — `w2-window_manager-part1` (W2-window_manager part 1, `src/window_manager.h` + `src/window_manager.c:1-700`)

`src/window_manager.c:14` | `compare_wm` compared two `uint32_t` keys through `void *` for `struct table` | not translated (`DECISIONS.md` 5 and 16, `state-access/window_manager.md` §2); `Table<K, V>` compares with `K: PartialEq`, so the seven `table_init` comparison arguments disappear with it

`src/window_manager.c:9` | one `hash_wm` served tables keyed by `uint32_t` and by `pid_t` through `void *` | two functions with the C body, `hash_wm_window_id(&WindowId)` and `hash_wm_process_id(&ProcessId)` (`DECISIONS.md` 45); the `pid_t` form keeps the C reinterpretation with `key.0 as u32 as u64`

`src/window_manager.c:63-88` | `window_manager_query_windows_for_displays` concatenated every display's space list by keeping only the **first** `display_space_list` pointer and summing the counts, relying on the `ts` bump allocator laying consecutive allocations out contiguously | the lists are concatenated into one owned `Vec<SpaceId>` (`files/window-manager.md` §3.3 and hazard 13, `DECISIONS.md` 17); the carried-over comment at `:78-82` stays at the matching place even though the allocator it describes is gone

`src/window_manager.c:91-106` | `window_manager_rule_matches_window` passed `window->application->name` and the three `char *` arguments straight to `regexec` | the `Arc<str>` and the three `&str` are converted with `CString::new(..).unwrap()` at each call, because `regex_match` takes `&CStr` (`DECISIONS.md` 44); a `Rule`'s `*_regex_valid` flag is the `Option<PosixRegex>` being `Some`

`src/window_manager.c:160-163` | `window_manager_apply_rule_effects_to_window` `free`d the `string_copy(effects->scratchpad)` when `window_manager_set_scratchpad_for_window` returned false | the `String` is moved into the callee, which drops it on the false path, so the `if (!..) free(..)` guard has no Rust counterpart

`src/window_manager.c:171-217` | the two rule loops indexed `wm->rules[i]` while passing `wm` to `window_manager_rule_matches_window` | the `Vec<Rule>` is moved out with `std::mem::take` for the duration of the loop and put back before the effects are applied; nothing the loop calls reads `WindowManager::rules`, so the temporary emptiness is unobservable

`src/window_manager.c:19`, `:242`, `:272`, `:330`, `:346`, `:368`, `:415`, `:425` and the rule paths | every `struct window *`, `struct application *` and `struct view *` was dereferenced without a NULL check | each becomes a table lookup that returns the neutral value, returns early or `continue`s on a miss (`DECISIONS.md` 14, `patterns/state-and-ownership.md` §4.1)

`src/window_manager.c:437-460` | `window_manager_notify_jankyborders` wrote `animation_count` entries into two fixed `uint32_t[512]` arrays with no bound, overrunning the stack struct for a batch of more than 512 windows | the fill loop stops once 512 entries have been written (`files/window-manager.md` §5 hazard 2, `DECISIONS.md` 4)

`src/window_manager.c:550-554` | the `switch (context->animation_easing)` had no `default`, so an out-of-range easing value left `float mt` uninitialised and the frame used garbage | `AnimationEasingType::apply` is total over the 21 variants (`files/window-manager.md` §5 hazard 3, `DECISIONS.md` 4)

`src/window_manager.c:560-563` vs `:635-642` | `proxy.tx/ty/tw/th` were plain `float`s written every frame by the CVDisplayLink thread and read under the lock by the event-loop thread — a data race | the four become `AtomicU32` holding `f32::to_bits`, relaxed both ways (`DECISIONS.md` 24, `GLOSSARY.md` §3.13); the values and the `(int)` truncation at `:635-642` are unchanged, except that the supersede branch loads each of the four once and reuses it for both the `frame` and the `t*` assignment, where the C read the racing field twice and could see two different values

`src/window_manager.c:648` | `__asm__ __volatile__ ("" ::: "memory")` | `std::sync::atomic::compiler_fence(Ordering::SeqCst)` — the compiler barrier, not a hardware fence, which would change timing

`src/window_manager.c:609`, `:631`, `:662`, `:673` | `window_animations_table` stored `&context->animation_list[i]`, an interior pointer into a `malloc`ed array that the superseded batch's display-link refcon kept alive | the value is `(Arc<AnimationContext>, usize)`; the C's `table_find` at `:631` becomes `Table::remove`, which detaches the entry and hands it over as an owned local, and the C's `table_remove` at `:662` becomes the explicit `drop` of that local at the same position (`THREADS.md` §8.2, `patterns/state-and-ownership.md` §4.5)

`src/window_manager.c:507`, `:585`, `:652`, `:663`, `:666` | the builder threads, the supersede branch and the display-link callback all wrote `proxy.id`, `proxy.context`, `proxy.image`, `proxy.frame`, `proxy.level` and `proxy.sub_level` through interior pointers into the shared batch | the same entries are reached through `Arc::as_ptr(..).cast_mut()` in the module-private `window_animation_in_batch_at_index`, because `GLOSSARY.md` §3.13 keeps those six fields plain; the writers remain disjoint exactly as in C — one builder per index before the join, the supersede branch under the lock, the destroy loop under the lock

`src/window_manager.c:615`, `:666`, `:669`, `:680` | `ts_alloc_list(pthread_t, window_count)` plus `pthread_create` inside the locked loop and `pthread_join` after it, falling back to running the builder inline when `pthread_create` failed | `std::thread::scope` with `Builder::spawn_scoped` per index and the same inline fallback on `Err`; the builders are spawned after the lock guard is dropped rather than inside the loop, and the closing brace of `thread::scope` is the join point of `:680` (`THREADS.md` §8.2)

`src/window_manager.c:592-593`, `:596` | `free(context->animation_list); free(context);` and `CVDisplayLinkRelease(link)` | the refcon's `Arc` is reconstructed with `Arc::from_raw` and dropped at the position of the two `free`s, and the display link is released by dropping a `CFRetained<CVDisplayLink>` at the position of `CVDisplayLinkRelease`, which `objc2-core-video` does not expose as a function

`src/window_manager.c:418-422`, `:428-432` | `AXValueCreate` followed by an explicit `CFRelease` | the `CFRetained<AXValue>` is dropped explicitly at the C's `CFRelease` position

`src/window_manager.c:524`, `:734` | `AX_ENHANCED_UI_WORKAROUND(r, c)`, a macro wrapping a statement block | `with_enhanced_user_interface_disabled(reference, || { .. })` from `src/ffi/accessibility.rs`, whose `RestoreEnhancedUserInterfaceOnDrop` guard restores the attribute where the macro's trailing `if (eui)` did

`src/window_manager.c:700-702` | `CVDisplayLinkCreateWithActiveCGDisplays(&link)` left `link` uninitialised on failure and the next two calls used it | the display link is only installed and started when the create call produced a non-null pointer (`DECISIONS.md` 4)
