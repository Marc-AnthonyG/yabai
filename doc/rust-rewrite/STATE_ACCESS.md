# STATE_ACCESS — the wave 0b signature contract

`TRANSLATION_PLAN.md` §3.2 asks wave 0b for one table with a row per C function: C name,
`file:line`, thread context, the event-loop-owned state it touches transitively, and the final
Rust parameter list. That table is nine documents in `state-access/`, one per Rust module, each
written against the C files it owns. This file is their index and the record of the
cross-module pass that reconciled them.

Wave 1 pastes the `Rust signature` column of those nine documents verbatim. Nothing here
restates a signature; the nine files are the contract.

---

## 1. The nine documents

| Document | C sources it owns | Rows | Translated | Recorded as not translated |
| --- | --- | ---: | ---: | ---: |
| [`state-access/display.md`](state-access/display.md) | `src/display.c`, `src/display_manager.c` | 47 | 44 | 3 |
| [`state-access/event_loop.md`](state-access/event_loop.md) | `src/event_loop.c`, `src/mission_control.c` | 50 | 50 | 0 |
| [`state-access/message.md`](state-access/message.md) | `src/message.c` | 31 | 30 | 1 |
| [`state-access/signal-rule-mouse.md`](state-access/signal-rule-mouse.md) | `src/event_signal.c`, `src/rule.h`, `src/rule.c`, `src/mouse_handler.c` | 38 | 37 | 1 |
| [`state-access/space.md`](state-access/space.md) | `src/space_manager.c`, `src/space.c` | 74 | 69 | 5 |
| [`state-access/view.md`](state-access/view.md) | `src/view.h`, `src/view.c` | 52 | 52 | 0 |
| [`state-access/window-application-process.md`](state-access/window-application-process.md) | `src/window.h`, `src/window.c`, `src/application.h`, `src/application.c`, `src/process_manager.h`, `src/process_manager.c` | 71 | 69 | 2 |
| [`state-access/window_manager.md`](state-access/window_manager.md) | `src/window_manager.c` | 122 | 122 | 0 |
| [`state-access/workspace-sa-main.md`](state-access/workspace-sa-main.md) | `src/workspace.h`, `src/workspace.m`, `src/sa.m`, `src/yabai.c` | 68 | 68 | 0 |
| **Total** | | **553** | **541** | **12** |

Row counts are of signature rows in each document's own tables; a document that also lists a
dead function in a separate table counts it once. `state-access-closure.py` extracts 535 function
definitions from those C files and 645 from all of `src/` outside `src/osax/`; the difference is
`src/misc/**`, which belongs to the `misc` unit and has no document here. Rows outnumber extracted
definitions where a document also covers header helpers the extractor does not see as definitions
(the six `rule.h` inline flag helpers, the three `view.h` macros, the six `window.h` flag
accessors) and the ten Objective-C methods of `workspace_context`.

---

## 2. The rules in force

Everything below is from `DECISIONS.md` and the pattern documents; nothing here is new.

1. **Parameter sets come from the transitive call graph, once** (`DECISIONS.md` 13,
   `patterns/state-and-ownership.md` §2.1). `Managers(f)` is the least fixed point of
   `Direct(f) ∪ ⋃ Managers(callee)`. A caller and a callee that disagree mean the fixed point was
   computed wrong, not that a signature should be patched locally.
2. **`Direct(f)` includes handle resolution**, not only `g_*` names: a `SpaceId` resolved through
   `SpaceManager::view` contributes `SpaceManager`, a `WindowId` through `WindowManager::window`
   contributes `WindowManager`. `state-access.tsv` is textual and under-reports this; it is a
   starting point, not the answer.
3. **Order** (`patterns/state-and-ownership.md` §2.2). C parameters first, in C order, with each
   pointer rewritten to its handle; a manager the C declares keeps its C position; `struct view *`
   becomes `&mut SpaceManager` plus the `SpaceId` at that position; everything else is **appended
   after the whole C parameter list**, in the `EventLoopOwnedState` field order of §1.1:

   `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`,
   `signal_storage`, `mouse_drag_state`, `mission_control_mode`,
   `focus_follows_mouse_suspended_value`, `is_menu_open`.

   Restricted to the five managers §2.2 step 3 names, this is byte for byte that section's order;
   `TRANSLATION_PLAN.md` §3.2 fixes the full ten, which is what the eight non-manager
   `EventLoopOwnedState` fields need.
4. **Every state parameter is `&mut`** (`patterns/state-and-ownership.md` §2.1). No read-only
   variants, including for the three `Copy` scalars.
5. **Statics are never parameters**: `CONNECTION`, `VERBOSE`, the three window levels, the paths,
   `PROCESS_TABLE`, `MOUSE_TAP_STATE`, `EVENT_SENDER`, `WORKSPACE_CONTEXT`, the four
   `__pending_*` atomics.
6. **No main-thread callback and no display-link callback takes any of it** (`DECISIONS.md` 20).
   Start-up functions that main runs against `&mut state` before the event-loop thread is spawned
   are not callbacks and do take managers: `window_manager_init`, `window_manager_begin`,
   `space_manager_begin`, `display_manager_begin`, `process_manager_begin`,
   `process_manager_add_running_processes`.
7. **Handles, not pointers** (`DECISIONS.md` 14, `GLOSSARY.md` §1). `WindowId`, `ProcessId`,
   `SpaceId`, `DisplayId`, `NodeId`, `(SpaceId, NodeId)` across a `View` boundary. The one
   exception this pass confirmed is in §4 below.
8. **One spelling per thing** (`GLOSSARY.md`, `DECISIONS.md` 37). The glossary wins over any
   pattern document's sketch.

### How the contract was checked

Mechanically, against the C, on the finished nine documents:

* every one of the 512 rows whose C name the closure script extracts was parsed into its
  parameter list, and each call edge in the C was tested for `State(callee) ⊆ State(caller)`,
  iterated to a fixed point — **0 remaining**;
* the appended tail of every row was tested against the canonical order, with C-declared managers
  excluded — **0 violations**;
* every row tagged main-thread or display-link was tested for state parameters, and every such
  row's callees for state they would demand — **0 violations**;
* every state parameter was tested for `&mut` — **0 violations**;
* every function mentioned in a document other than the one that owns it was compared parameter
  for parameter and return type against the owning row — **3 remaining, all deliberate**: two
  quotations of a pattern document's older sketch (`display.md` judgement 5,
  `window-application-process.md` §6 on `view_find_window_node`) and one call form rather than a
  signature (`window_manager.md` on `event_loop_post`).

---

## 3. Every change this pass made, and why

### 3.1 `mission_control_mode` became a parameter — 22 rows

`state-access/event_loop.md` and `state-access/message.md` gave
`mission_control_is_active(mission_control_mode: &mut MissionControlMode)` and propagated it;
`state-access/display.md`, `state-access/space.md` and `state-access/window_manager.md` each
argued it contributes nothing, because `patterns/state-and-ownership.md` §2.2 step 3 enumerates
five managers and `MissionControlMode` is not one.

**Settled for the parameter.** §2.2 step 3 lists five managers because §2.1 defines `Managers(f)`
over managers; `TRANSLATION_PLAN.md` §3.2 step 3 — the spec for this wave — names
`g_mission_control_mode` among the eight event-loop-owned globals to scan for, and its row format
fixes the append order as the `EventLoopOwnedState` field order, in which `mission_control_mode`
(`yabai.c:37`) is the last appendable field. `DECISIONS.md` 13 then forbids
`mission_control_is_active` (`mission_control.c:108`, which reads `g_mission_control_mode`
directly) from reaching it through a global. `state-access.tsv` lists the global transitively for
45 functions; every one of them that is not `main` now takes the parameter.

| Document | Rows that gained it |
| --- | --- |
| `display.md` | `display_manager_focus_space` |
| `signal-rule-mouse.md` | `rule_apply`, `rule_reapply_all`, `rule_reapply_by_index`, `rule_reapply_by_label` |
| `space.md` | `space_manager_toggle_mission_control`, `_toggle_show_desktop`, `_swap_space_with_space`, `_move_space_to_space`, `_move_space_to_display`, `_focus_space`, `_switch_space`, `_destroy_space`, `_add_space`, `_refresh_application_windows` |
| `window_manager.md` | `window_manager_apply_rule_effects_to_window`, `_apply_rules_to_window`, `_create_and_add_window`, `_add_application_windows`, `_add_existing_application_windows`, `_scratchpad_recover_windows`, `window_manager_begin` |

### 3.2 `mouse_drag_state` became a parameter — 29 rows

`patterns/state-and-ownership.md` §5.4 writes `view_free_node` with a
`mouse_drag_state: &mut MouseDragState`, because it clears `MouseDragState::feedback_node` when a
node slot is freed — something the C never does. `state-access/view.md` N12 and
`state-access/signal-rule-mouse.md` §8.7 both derived the consequence and propagated it as far as
their own rows; four sibling documents stopped short, which left the fixed point broken at
`view_remove_window_node`, `window_node_destroy`, `view_clear` and `view_destroy`.

**Settled for propagating.** §5.4's clearing is not cosmetic: the §5.1 arena recycles a freed
`NodeId` through `View::free_node_ids`, so a stale `feedback_node` would not merely dangle as it
does in C — it would name a *different* live node. Dropping the clear was the only alternative,
and it is a pattern-document code block this pass cannot edit.

| Document | Rows that gained it |
| --- | --- |
| `event_loop.md` | `EVENT_HANDLER_APPLICATION_HIDDEN`, `_WINDOW_MINIMIZED`, `_SLS_SPACE_DESTROYED`, `_DISPLAY_ADDED`, `_DISPLAY_REMOVED`, `_MISSION_CONTROL_EXIT` |
| `message.md` | `handle_domain_config`, `handle_domain_space`, `handle_domain_rule` |
| `space.md` | `space_manager_untile_window`, `_set_layout_for_space`, `_set_layout_for_all_spaces`, `_destroy_space` |
| `window_manager.md` | `window_manager_apply_manage_rule_effects_to_window`, `_apply_rule_effects_to_window`, `_apply_manage_rules_to_window`, `_apply_rules_to_window`, `_stack_window`, `_warp_window`, `_send_window_to_space`, `_make_window_floating`, `_make_window_sticky`, `_set_scratchpad_for_window`, `_remove_scratchpad_for_window`, `_validate_windows_on_space`, `_check_for_windows_on_space`, `_validate_and_check_for_windows_on_space`, `_correct_for_mission_control_changes`, `_handle_display_add_and_remove` |

`message.md`'s judgement that every `g_mouse_state` *name* reachable from `handle_domain_config`
is in the `MOUSE_TAP_STATE` half (`DECISIONS.md` 23) survives intact — the parameter arrives by a
path no `g_mouse_state` grep can find, through `view_clear`.

### 3.3 Ordering — 3 rows

`EVENT_HANDLER_MISSION_CONTROL_EXIT`, `handle_domain_space` and `handle_domain_rule` already had
`mission_control_mode`; the new `mouse_drag_state` was inserted before it, per rule 3.
The "state touched" column was reordered to match on the same rows.

### 3.4 One C type, one Rust type

| Change | Rows | Why |
| --- | --- | --- |
| `label: &str` → `label: &[u8]` on `rule_reapply_by_label`, `rule_remove_by_label`, `event_signal_remove`, `signal_type_from_string`, `space_manager_get_space_for_label`, `window_manager_find_scratchpad_window`, `window_manager_toggle_scratchpad_window_by_label` | 7 | Every value reaching them is token text. `patterns/message-and-serialisation.md` §3 names `message.c:772`, `:853`, `:2849`, `:2963` and types the replacement `c_string_at(message_bytes, value.token.start)` as `&[u8]`; `message.c:2332`, `:2832`, `:2928` are the same shape. `display_manager_get_display_for_label` already had it. The two callers that hold a stored `String` (`rule.c:177`, `event_signal.c:354`) pass `.as_bytes()` |
| `Option<CFRetained<CFString>>` → `Option<CFStringOwned>` in `display.md` | 5 signatures + 2 prose | `GLOSSARY.md` §2.5 names the owned `CFStringRef` `CFStringOwned` and the glossary is the spelling authority. `window-application-process.md` and `view.md` already spelt it that way. It is an alias, not a third wrapper, so `patterns/memory-text-and-os-objects.md` §1.1's "exactly two wrapper types" is unaffected; `application_window_list`'s `CFRetained<CFArray>` has no alias and keeps its spelling |
| `display_space_list(..) -> Vec<SpaceId>` → `-> Option<Vec<SpaceId>>` | 1 | `display.c:218-240` leaves the list `NULL` when the display's uuid is not found, but allocates a zero-length block when it is found with no spaces, and `space_manager.c:52` returns `false` on `NULL` while printing `[]` on empty. The distinction is observable, so `DECISIONS.md` 32 applies. `space.md` had it right |
| `window_title` / `window_role` / `window_subrole` parameters `Option<&str>` → `&str` on `window_manager_rule_matches_window`, `_apply_manage_rules_to_window`, `_apply_rules_to_window` | 3 | `window_{title,role,subrole}_ts` return `ts_string_copy("")` on a missing value (`window.c:726`, `:1004`, `:1025`), and every value that reaches these parameters is one of those results (`rule.c:121`, `:125`, `:163`, `window_manager.c:178`, `:202`, `:1487`, `:1491` — all of them). `window-application-process.md` types the three `_ts` functions `-> String`; the parameter now matches |
| `application_notification_handler(observer: AXObserverRef, element: AXUIElementRef, notification: CFStringRef, ..)` → `NonNull<AXObserver>`, `NonNull<AXUIElement>`, `NonNull<CFString>` | 1 | `patterns/ffi-objc-and-os.md` §19.2 gives both AX observer callbacks that shape, read out of `objc2-application-services`, and a plain `extern "C" fn` will not coerce to `AXObserverCallback`. `event_loop.md`'s `mission_control_notification_handler` already had it; the two are the same C typedef |
| `window_manager_animate_window_list_thread_proc`: `CVDisplayLinkRef` / `*const CVTimeStamp` / `*mut CVOptionFlags` → `NonNull<CVDisplayLink>` / `NonNull<CVTimeStamp>` / `NonNull<CVOptionFlags>` | 1 | `patterns/ffi-objc-and-os.md` §19.6, same reason |
| `extern "C"` → `extern "C-unwind"` on `application_notification_handler`, `process_handler`, `window_manager_animate_window_list_thread_proc` | 3 | `patterns/ffi-objc-and-os.md` §19 rule 2: every function Rust *exports* to C is `unsafe extern "C-unwind"`, all seven of them. The other four already were |
| `connection_id: c_int` → `i32` | 1 | `GLOSSARY.md` §10.2 types `cid` as `i32`; `space.c` and `window_manager.c` rows already did |

### 3.5 Spellings (`GLOSSARY.md`)

| Change | Where | Why |
| --- | --- | --- |
| `a_window_id` / `b_window_id` → `a_window` / `b_window` | `window_manager_stack_window`, `_warp_window`, `_swap_window` in `window_manager.md`, and the three matching call shapes in `message.md` | §10.3 names `window_manager.c:1799`, `:1832`, `:1950` by line and fixes `a_window` / `b_window` for the bare C `a`/`b`. §10.2's `a_window_id` row is a different C spelling at a different site (`a_wid`/`b_wid`, `sa.m:576`); the more specific rule wins |
| `window_element_ref` → `window_ref` | `window_manager_focus_window_with_raise` in `display.md` and `message.md` | §10.3 keeps `_ref` names that already name a CF value, and `window_ref` is listed there verbatim. §10.4's `element_ref` renames the bare field `ref`, which this is not. `window_manager.md` had it right |
| `r#type` → `notification_type`, `cid` → `connection_id` | `connection_handler` in `event_loop.md` | §11: "**No `r#type` anywhere**", and its table maps `connection_callback`'s `uint32_t type` to `notification_type`. §10.2 maps `cid` |
| `acting_display_id` → `source_display_id` | `display_manager_find_closest_display_in_direction` call shape in `message.md` | The definition (`display_manager.c:266`) names it `source_did`; `display.md` owns the row |
| `space_id` → `destination_space_id` | `window_manager_send_window_to_space` call shape in `message.md` | The definition (`window_manager.c:2092`) names it `dst_sid`; §10.2 maps `dst_sid` |

### 3.6 Record versus handle — 12 cross-module entries corrected

`patterns/state-and-ownership.md` §2.2 step 1 rewrites every `struct window *` and
`struct application *` parameter to a handle. `window-application-process.md` §1 shows that this
cannot hold for `src/window.c` and `src/application.c`, because the C calls most of those
functions on a record that is **not in its manager's table at that moment** — `window_create`
(`window.c:1100-1130`) calls `window_ax_frame`, `window_ax_role`, `window_title`,
`window_is_minimized`, `window_ax_can_move`, `window_ax_can_resize` and `window_is_fullscreen` on
a record `window_manager_add_window` does not insert until `window_manager.c:1471`. `THREADS.md`
§5.4 fixes `window: &mut Window` for `window_observe` / `window_unobserve` and
`patterns/idioms-and-conventions.md` §6.2 fixes `window: &Window` for the flag accessors.

**Settled for the record**, and `window_manager.md` §5 and `event_loop.md` §6 were corrected to
match: `window_title_ts` / `_role_ts` / `_subrole_ts` take `&Window` and return `String`;
`window_observe` / `window_unobserve` take `(&mut Window, window_manager)`; `window_ax_frame` and
the nine `window_is_*` / `window_can_*` predicates take `&Window` and no manager; `window_create`
returns `Window`, not `Option<Window>`, and `application_create` returns `Application`;
`application_observe` / `_unobserve` take `&mut Application`, `application_destroy`,
`_focused_window`, `_is_frontmost` and `_window_list` take the record. The `uint32_t wid`
functions keep the handle, and so do the `struct window *` parameters inside `window_manager.c`
itself, which always come out of the table.

`window_manager_focus_window_with_raise`'s ordering note in `THREADS.md` §5.4
(`(window_manager, window)`) was **not** adopted: §2.2 step 3 appends managers after the whole C
parameter list, and that section is the order authority.

### 3.7 `window_serialize` gained `display_manager` in two cross-module lists

`patterns/state-and-ownership.md` §2.3 worked example (2) computes the set by hand as
`{ SpaceManager, WindowManager, MouseDragState }`. It is one short: `window.c:533` calls
`display_manager_display_id_arrangement`, and `state-access.tsv` lists `g_display_manager` in the
row. `window-application-process.md` had the full set; `message.md`'s and `window_manager.md`'s
expectations did not, and were corrected to
`(response, window_id, flags, display_manager, window_manager, space_manager, mouse_drag_state)`.

### 3.8 Return shapes aligned to the owning document — 6 entries

| Function | Owner's shape | What the consumer said | Why the owner wins |
| --- | --- | --- | --- |
| `view_create` | `-> SpaceId` (`view.md` N10) | `-> View` (`space.md`) | `view.c:1010` calls `view_update`, which takes `(space_manager, space_id, ..)`, so the `View` must already be in the table; the insertion moves inside `view_create` |
| `view_add_window_node_with_insertion_point` | `-> Option<NodeId>` (`view.md`) | `-> NodeId` (`space.md`) | `view.c:811` really does `return NULL`; `DECISIONS.md` 32 |
| `window_manager_find_window` | `-> Option<WindowId>` (`window_manager.md`) | `-> Option<&mut Window>` (`space.md`) | `patterns/state-and-ownership.md` §3.2: a returned record pointer becomes `Option` of its handle |
| `window_space_list` | `-> Vec<SpaceId>` (`window-application-process.md`) | `-> Option<Vec<SpaceId>>` (`space.md`) | `window.c:89-111` returns `NULL` exactly when the count is zero, so an empty `Vec` models it exactly |
| `display_manager_active_display_list` | `-> Vec<DisplayId>` (`display.md`) | `-> Option<Vec<DisplayId>>` (`space.md`) | `display_manager.c:398-403` always returns the allocated block; there is no `NULL` |
| `display_manager_get_display_for_label`, `space_manager_get_space_for_label` | `-> Option<&mut DisplayLabel>` / `Option<&mut SpaceLabel>` | `-> Option<DisplayId>` / `Option<SpaceId>` (`message.md`) | `struct display_label` / `struct space_label` are not among the five record kinds `DECISIONS.md` 14 names, and neither of the two call sites holds the result past one expression |

### 3.9 Cross-module call shapes regenerated — 41 entries

Every `name(..)` in a "calls this module makes" or "what this module expects" section that did not
match the owning document's row, parameter for parameter and return type, was rewritten from that
row. Four were left alone on purpose and are listed at the end of §2.

### 3.10 Judgement-call prose rewritten where the verdict flipped

`display.md` 5 and 7; `space.md` 1 and 2; `window_manager.md` §4's derivation note and judgements
4 and 11; `signal-rule-mouse.md` 6, plus two new entries 7 and 8 with the rest renumbered;
`message.md` 4. Each now states what was settled, which document it was settled against, and the
C or contract line that settled it, so the reasoning is not lost.

`space.md` judgement 2's conclusion (no `signal_event` / `signal_storage` parameters in that
module) was kept but re-founded: its premise was that §2.2 step 3 omits them, which
`event_loop.md` §1 contradicts; the real reason is that `event_signal_push` is called from
`src/event_loop.c` alone and neither global appears in any `space_manager.c` or `space.c` row of
`state-access.tsv`.

---

## 4. Still open

These are disagreements this pass could not close inside the nine files. Each needs a decision
before or during wave 1.

1. **`window_serialize` is called with `stdout`.** `window_manager.c:1531` and `:1547`, both in
   `g_verbose` branches, pass `stdout` where every other caller passes the client socket.
   `patterns/message-and-serialisation.md` §7.1 gives `Response` a `UnixStream` and nothing else.
   Either `Response` grows a stdout case or those two calls become `debug!`. Owner: `W1-misc`
   with `W1-window`.
2. **`hash_wm` has no one key type.** `window_manager.c:2727-2733` installs it on seven tables
   whose keys are `ProcessId` (`i32`) and `WindowId` (`u32`). `patterns/state-and-ownership.md`
   §1.3 writes `hash_window_manager_key(key: &u32)`; `GLOSSARY.md` §3.26 declares the field
   `hash: fn(&K) -> u64`. `space.md` solved the analogous `hash_view_key` by typing it `&SpaceId`,
   which works because `SpaceManager::view` has one key type. `W1-1` must make the window-manager
   one generic or mint a second. Recorded in `window_manager.md` judgement 12.
3. **The display-link callback needs `window_animations_table` and has no way to reach it.**
   `window_manager_animate_window_list_thread_proc` takes no manager (`DECISIONS.md` 20) yet the C
   body locks and mutates `g_window_manager.window_animations_table` (`window_manager.c:577-589`).
   `GLOSSARY.md` §3.1 already makes the field `Arc<Mutex<..>>`, so a clone must travel in the
   `AnimationContext` — but `GLOSSARY.md` §3.15 has no field for it. Owner: `W1-window_manager`.
   Recorded in `window_manager.md` §3.
4. **`regex_match` has three shapes and none is in these nine files.**
   `patterns/message-and-serialisation.md:1731` writes `(regex: Option<&PosixRegex>, subject: &CStr)`;
   `sweeps/c-idioms-and-semantics.md:840` writes `(valid: bool, regex: &PosixRegex, match_: &CStr)`;
   the C is `regex_match(bool valid, regex_t *regex, const char *match)`. It lives in
   `src/misc/helpers.h`, which no document here owns. `window_manager.md`'s expectation now cites
   both and defers. Owner: `W1-misc`.
5. **Thread-context vocabulary is not uniform.** `event_loop.md` uses `THREADS.md` §1.1's tags
   (`MAIN`, `EVENTLOOP`, `MSGLOOP`, `CVLINK`, `PROXY`); the other eight use prose ("event loop",
   "main run loop", "display link", "start-up only"). No signature depends on it, so it was left
   alone rather than churned across 553 rows. If wave 1 wants one vocabulary, `THREADS.md` §1.1
   is the list, and it has nine tags, not six.
6. **Visibility policy.** All nine documents set `pub(crate)` from the tsv's
   `called_from_other_files` column, which makes several header-declared functions private and is
   one `DEVIATIONS.md` line each. `patterns/idioms-and-conventions.md` §2 instead sets a blanket
   `pub(crate)` for every function. `event_loop.md` judgement 11 flags this; it is a policy call,
   not a per-row one, and it changes no parameter list.
7. **`patterns/memory-text-and-os-objects.md` §2.2 lists `window_manager.c:180-205` among the
   "genuinely nullable" `string_equals` sites.** The C type is nullable there; the values are not
   (§3.4 above). Those sites now take that section's *second* form, plain `left == right`. If the
   pattern document meant something the C does not show, `window_manager.md` judgement 11 is the
   row to revisit.
8. **Two pattern-document sketches are now known to be short.**
   `patterns/state-and-ownership.md` §2.3 example (2) omits `DisplayManager` from
   `window_serialize` (§3.7), and §2.3 example (6) gives `window_manager_begin` two managers where
   the fixed point gives six (`window_manager.md` judgement 2, extended here by
   `mission_control_mode`). Neither is editable from this unit; both are worth a line in whatever
   errata the gate keeps.
