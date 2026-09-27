# Binding decisions for the Rust rewrite

These settle every disagreement between the inventories in `files/` and `sweeps/`. Where an
inventory recommends something else, this file wins. Every other document in this folder
elaborates these decisions; none may reopen them.

## Scope and fidelity

1. `src/osax/` stays C and is never edited. Everything else under `src/` becomes Rust,
   including `src/sa.m`, `src/workspace.m`, `src/misc/notify.h` and `src/misc/service.h`.
2. Phase 2 transposes one C pair into one Rust module, keeping function names so the Rust can
   be read against the C. Phase 3 splits and renames; phase 2 does not.
3. "Behaviour identical" means every externally observable, *defined* behaviour: socket wire
   format, query JSON byte for byte (including the asymmetric `[`/`]` quirks), CLI flags and
   exit codes, `YABAI_*` environment variables, `/tmp` paths, lock file semantics, the launchd
   plist text, the scripting-addition frame layout, window placement arithmetic to the pixel,
   and every timing threshold including the `f32` timer deltas.
4. Undefined behaviour is never reproduced: buffer overruns, use after free, data races,
   uninitialised reads, leaks, NULL dereferences. Each becomes the safe equivalent (bounds
   check, early return, atomic, owned value) and the translator records it as one line in
   `doc/rust-rewrite/DEVIATIONS.md`: C location, what C did, what Rust does.
5. Dead code is not translated: `misc/autorelease.h`, the `PROFILE` machinery in
   `misc/timer.h` (`TIME_FUNCTION`, `PROFILER_END_TRANSLATION_UNIT`, anchors; the clock readers
   that live code calls stay), declarations with no definition or no caller, `#if 0` branches,
   `daemon_deprecated`. Each removal is one line in `DEVIATIONS.md`.

## Crate and build

6. One binary crate at the repo root: `Cargo.toml`, `build.rs`, crate root `src/main.rs`,
   edition 2024, version `7.1.25`. Rust files sit next to the C files during phase 2; the C
   daemon sources and the `xxd` step are deleted only once the Rust daemon builds for both
   targets.
7. `panic = "unwind"` in every profile, plus a panic hook installed first thing in `main` that
   prints and calls `std::process::abort()`. Verified by spike: under `panic = "abort"` the
   process dies on the `removeObserver` exception yabai deliberately swallows; under `unwind`
   `objc2::exception::catch` catches it. A panic must still take the whole daemon down, never
   leave a dead event-loop thread behind a live process.
8. `overflow-checks = false` in every profile. Arithmetic that C relied on wrapping uses
   `wrapping_*` explicitly anyway.
9. `build.rs` compiles `src/osax/payload.m` and `src/osax/loader.m` with `xcrun clang` for
   `x86_64` + `arm64e` into `OUT_DIR`; the daemon embeds them with `include_bytes!`. It also
   emits the PrivateFrameworks search path, `MACOSX_DEPLOYMENT_TARGET=11.0`, the
   `-sectcreate __TEXT __info_plist` link argument with an absolute path, a complete
   `rerun-if-changed` list, and generates `OUT_DIR/osax_common.rs` from `src/osax/common.h`,
   failing the build if the header cannot be parsed. Verified by spike: all of this works and
   the lipo'd binary carries the plist in both slices.
10. The makefile keeps every user-facing target and drives two `cargo build --target`
    invocations plus `lipo`. `asan`/`tsan` become host-only nightly targets. No CI is added.
11. Dependencies: `objc2`, `objc2-foundation`, `objc2-app-kit`, `objc2-core-foundation`,
    `objc2-core-graphics`, `objc2-application-services`, `objc2-core-video`, `block2`,
    `dispatch2`, `libc`. Nothing else: no `regex`, `serde`, `bitflags`, `crossbeam`,
    `indexmap`, `bumpalo`, `bindgen`, `mach2`, and no servo `core-foundation`/`core-graphics`.
    SkyLight, Carbon, ColorSync's two functions, the `kAX*` string constants, Mach message
    structs and the Mach-O symbol-table types are hand-declared under `src/ffi/`.

## State and ownership

12. Everything the event-loop thread owns lives in one struct, `EventLoopOwnedState`, with the
    managers as fields. It is built on the main thread in the C start-up order, then *moved*
    into the event-loop thread, which is spawned after `window_manager_begin` and
    `update_window_notifications` instead of at the C position. The channel exists from
    `event_loop_begin` onward, so events posted during start-up queue and are handled in order.
    `unsafe impl Send` on the state, justified in `THREADS.md`.
13. Functions take the managers they touch as explicit `&mut` parameters, in the C parameter
    order, with any manager C reached through a global appended. No function reaches
    event-loop-owned state through a global. The parameter sets are computed once from the
    transitive call graph before any body is written, so callers and callees agree.
14. No long-lived pointers into owned collections. Windows, applications, views and tree nodes
    are referred to by handle (window id, pid, space id, `(space id, NodeId)`) and looked up at
    each use. A lookup miss replaces what would have been a dangling pointer in C.
15. The BSP tree is an index arena owned by its `View`: nodes in a `Vec`, `NodeId` handles for
    `parent`/`left`/`right`/`zoom`, the root always `NodeId` 0 so the C "reset the root in
    place" idiom keeps working.
16. `misc/hashtable.h` is ported as a safe generic `Table` that keeps the C hash functions,
    bucket iteration order and the "add does not overwrite" rule. `std::collections::HashMap`
    is not used for any table the C iterates.
17. Replaced rather than ported: the `ts` arena and `ts_buf_*` become `Vec`/`String` (the three
    sites that concatenate consecutive arena allocations extend one `Vec`); `memory_pool` and
    `g_signal_storage` become owned queues; `buf_*` becomes `Vec` with `swap_remove` wherever C
    used `buf_del`. Queued signals own their strings.
18. Process-wide values written once before any thread starts (`g_connection`, `g_pid`, the
    three window levels, the clock frequency, the socket/lock/config paths, the bootstrap port,
    the two runtime-resolved SkyLight function pointers) are `OnceLock` statics. `g_verbose` is
    an `AtomicBool`.

## Threads

19. The event queue is `std::sync::mpsc` carrying a typed `Event` enum whose variants own their
    payloads; `Drop` replaces the hand-written `CFRelease`/`free`/`close` per event type. FIFO
    order is preserved. The wrapping 512 KiB ring and the named semaphore are not reproduced.
    The autorelease pool is drained when the queue runs empty, as in C.
20. Main-thread callbacks never dereference event-loop-owned memory. An AX refcon carries
    either an integer id or a raw `Arc` pointer to an immutable-plus-atomic cell. That `Arc` is
    released on the main queue after the notification is removed, never directly from the
    event-loop thread.
21. The `id_ptr` CAS becomes a liveness cell shared between the `Window` and its AX refcon:
    claim-for-destruction is a compare-exchange from alive to dead, the nine probes are loads
    of the same cell. An event for a window that is dead or no longer in the table is dropped.
22. `struct process` is `Arc<Process>` with atomic `terminated`, `ns_application` and `policy`. The
    process table sits behind a `Mutex` that is never held across an ObjC or AX call (KVO with
    `NSKeyValueObservingOptionInitial` re-enters synchronously).
23. `g_mouse_state` splits in two: the half the event tap reads (`modifier`, the two actions,
    the drop action, the tap handle, the click-consumption flags) is a static of atomics; the
    drag half is owned by `EventLoopOwnedState`.
24. Animation: `Arc<AnimationContext>` shared with the CVDisplayLink callback, `skip` as
    `AtomicBool`, the raced `tx/ty/tw/th` as `AtomicU32` holding `f32` bits,
    `window_animations_table` behind an `Arc<Mutex<_>>` held for exactly the C lock scope,
    proxy builders under `std::thread::scope`.
25. Children are spawned with `libc::fork` + `execvp` so `SIGPIPE`/`SIGCHLD` dispositions are
    inherited exactly as in C. Everything the child needs (argv, environment, the regex filter
    verdict) is computed in the parent before the fork; the child only calls async-signal-safe
    functions and leaves through `_exit`. The double fork is kept.

## Text, numbers, formats

26. POSIX regex through `libc::regcomp`/`regexec`/`regfree` behind a `Drop` wrapper that boxes
    the `regex_t`. The three-valued match result stays three-valued.
27. Message tokens are `(start, length)` ranges over the mutable message buffer, tokenised by a
    hand-written cursor that reproduces `get_token` exactly. `strtof` and the nine `sscanf`
    sites call libc directly rather than being re-implemented.
28. Text is `String`; socket bytes become text with `from_utf8_lossy` at the point they are
    stored. Responses are written as bytes through one `Response` type that owns the failure
    prefix byte and the "no response wanted" case.
29. JSON is written with hand-rolled format strings, never a serialiser. Floats are cast to
    `f64` before formatting, `%d` of a `u32` prints `as i32`, `%lld` of a `u64` prints
    `as i64`, a NULL `%s` prints `(null)`.
30. Every implicit C numeric conversion becomes an explicit `as` cast at the same point in the
    expression, so `f32`/`f64`/`int` mixing, truncation and rounding happen exactly where they
    did. `struct area` stays `f32`.
31. Enums whose values are observable or used as indices keep explicit discriminants. Flag sets
    are newtypes with associated constants. X-macro lists become one `macro_rules!` each,
    generating the enum and its string table.
32. C `bool` returns stay `bool`, nullable pointers become `Option`, observable sentinels
    (`INT_MAX`, index 0) stay. `Result` appears only at `io::Write`.
33. `debug!`, `warn!`, `error!` (stderr, exit `EXIT_FAILURE`) and `require!` (stderr, exit
    `EXIT_SUCCESS` so launchd does not restart) are macros taking the C format text. `notify`
    stays a separate call, made only where C makes it. `__FUNCTION__` becomes the function
    name written as a literal. `assert` becomes `debug_assert!`.
34. Install and service paths keep their libc calls (`system`, `popen`, `posix_spawn` with a
    NULL environment, `_NSGetExecutablePath`, `open`/`fcntl` for the lock file whose descriptor
    is never closed). Scripting-addition frames are packed into a fixed 4096-byte buffer with
    an `i16` length header and a bounds check that fails the call.
35. Packed Mach structs are `#[repr(C, packed(4))]` with a `const` size assertion each.
36. `cgimage_restore_alpha` uses `std::arch` SSE2 and NEON intrinsics instruction for
    instruction; a 1 to 3 pixel tail goes through the same routine on a padded scratch copy.

## Names and comments

37. Functions and types keep their C names (`struct window_manager` becomes `WindowManager`,
    enum constants become CamelCase variants without the prefix). Fields, parameters and
    locals are never abbreviated: `window_manager` not `wm`, `window_id` not `wid`, `space_id`
    not `sid`, `display_id` not `did`, `process_id` not `pid`, `process_serial_number` not
    `psn`. One glossary fixes every spelling.
38. Only comments present in the C source are carried over, verbatim, at the matching place.
    No doc comments, no `SAFETY` comments, no comments describing the translation. Soundness
    arguments live in `THREADS.md`.
39. `unsafe` is confined to FFI calls, refcon and context casts, `Send`/`Sync` impls, packed
    Mach structs, the Mach-O symbol walker and the AX pid offset read.
40. The two existing C tests are transposed with the file they test. No other test is written.

## Rulings after wave 0

41. `GLOSSARY.md` is binding for every spelling and every type name. Where a pattern document,
    `THREADS.md` or an inventory spells something differently, the glossary wins.
42. Code on disk is ground truth for the API it defines. `src/ffi/`, `src/support/`,
    `src/service/` and `src/state/` exist and compile; later waves call what is there, not what a
    pattern document sketched. The dispatch wrapper is `dispatch_after_on_main_queue`.
43. Every C `FILE *rsp` parameter is `response: &mut Response`, in first position. The two
    verbose-mode calls that pass `stdout` (`src/window_manager.c:1531`, `:1547`) build the value
    with `Response::to_standard_output()`.
44. `regex_match` is the one in `src/support/regex.rs`:
    `regex_match(regex: Option<&PosixRegex>, subject: &CStr) -> RegexMatch`. Each C pair of
    `regex_t` and `*_regex_valid` becomes one `Option<PosixRegex>` field.
45. `hash_wm` becomes two functions with the C body, `hash_wm_window_id(&WindowId)` and
    `hash_wm_process_id(&ProcessId)`, because `Table::new` takes a `fn(&K) -> u64` and the seven
    tables have two key types.
46. `AnimationContext` carries its own clone of the `Arc<Mutex<_>>` holding
    `window_animations_table`. The display-link callback reaches the table through it and takes
    no manager.
47. `Process::policy` is an `AtomicI32`, the third atomic of decision 22: it is written from the
    event-loop thread and at start-up and read from the main-thread KVO callback.
48. Every function is `pub(crate)` in phase 2. Phase 3 tightens visibility when it splits modules.
49. Deviations are recorded per unit in `doc/rust-rewrite/deviations/<unit key>.md`, never in a
    shared file, because units run concurrently. They are merged into `DEVIATIONS.md` once, at
    the end of phase 2. Signature changes follow the same rule under `signature-changes/`.
50. `ax_application_notification` and `ax_application_notification_str` belong to
    `src/notifications/application.rs`. `WORKSPACE_CONTEXT` is declared by the unit that writes
    `src/notifications/workspace.rs`. `SIGNAL_TYPE_COUNT` moves from `src/state.rs` to
    `src/signal/definition.rs`.
51. `src/support/timer.rs` does not exist: its only live functions are `read_os_timer` and
    `read_os_freq` in `src/ffi/carbon_core.rs`. `Cargo.lock` is kept.
52. A re-export is added by the first unit that uses it, since an unused `pub use` is a warning.

## Changes to behaviour after the rewrite

These are deliberate improvements the user asked for. For the features they name they replace
decision 3's "behaviour identical to the C".

53. The insertion preview (`window --insert` and the drop target of a mouse drag) is a ghost of
    the exact frame the new or dropped window will occupy, computed with the same split, ratio and
    gap rules the tree applies when it inserts: a translucent rounded rectangle with a thin border,
    drawn only over that frame. Its colour follows the macOS accent colour until
    `insert_feedback_color` is set.
54. A pending insertion point is cleared when focus moves to a different window yabai already
    tracks. The next new window still consumes it, and repeating the same `--insert` still clears it.
55. One animator drives every window animation: one display link and one SkyLight connection.
    A window that is already moving keeps its proxy, and a new target is added as an additive layer,
    so the displayed frame is the target minus each layer's delta scaled by `1 - ease(t)` of that
    layer. `window_animation_duration` and `window_animation_easing` keep their meaning. Nothing
    on the display-link path blocks: the proxy swap-out, the JankyBorders notification and the
    proxy release run off it.
