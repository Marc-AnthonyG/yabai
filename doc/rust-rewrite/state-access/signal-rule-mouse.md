# State access — `src/event_signal.{h,c}`, `src/rule.{h,c}`, `src/mouse_handler.{h,c}`

Wave 0b unit `W0b-1`, module `signal-rule-mouse`. One row per C function defined in these six
files, in C source order, with the final Rust signature wave 1 pastes verbatim.

`src/event_signal.h` defines no function: it holds `enum signal_type`, the `signal_type_str`
table (`:47`), the three `SIGNAL_PROP_*` constants, `struct event_signal`, `struct signal` and
seven declarations. `src/event_signal.c` defines **10**. `src/rule.h` defines **6** `static
inline` flag helpers (`:59-65`) on top of its two enums, two structs and ten declarations.
`src/rule.c` defines **10**. `src/mouse_handler.h` defines no function: it holds the two event
masks, the `MOUSE_HANDLER` macro (`:21`), three enums, `struct mouse_window_info`,
`struct mouse_state`, the `mouse_mod_str` / `mouse_mode_str` tables and eleven declarations.
`src/mouse_handler.c` defines **12**.

**38 functions, 38 rows** — 37 translated, 1 dead.

---

## 1. The rule as applied here

`patterns/state-and-ownership.md` §2.2, unchanged:

1. the C parameter list, in C order, with every pointer rewritten to its handle (§3);
2. a manager the C declares keeps its C position — `struct window_manager *` and
   `struct space_manager *` become the matching `&mut`, `struct mouse_state *` becomes
   `&mut MouseDragState`, `struct view *` becomes `&mut SpaceManager` immediately followed by
   the `SpaceId` that replaces the pointer;
3. every remaining event-loop-owned member of `Touches(f)` is **appended after the whole C
   parameter list**, in the `src/yabai.c:27-37` declaration order, which is the
   `EventLoopOwnedState` field order of `GLOSSARY.md` §3.31:

| # | parameter | type |
| --- | --- | --- |
| 1 | `signal_event` | `&mut [Vec<Signal>; SIGNAL_TYPE_COUNT]` |
| 2 | `process_manager` | `&mut ProcessManager` |
| 3 | `display_manager` | `&mut DisplayManager` |
| 4 | `window_manager` | `&mut WindowManager` |
| 5 | `space_manager` | `&mut SpaceManager` |
| 6 | `signal_storage` | `&mut Vec<PendingSignal>` |
| 7 | `mouse_drag_state` | `&mut MouseDragState` |
| 8 | `mission_control_mode` | `&mut MissionControlMode` |

This is the same table `state-access/event_loop.md` §1 fixes, and rows 2-5 and 7 are exactly the
five managers §2.2 step 3 enumerates, in its order. Rows 1, 6 and 8 are the remaining
`EventLoopOwnedState` fields that this module reaches; `DECISIONS.md` 13 forbids reaching them
through a global, so they are parameters like the managers. Rows 1 and 6 are the whole subject of
`src/event_signal.c` and no signature in that file exists without them.

Everything else is a static and is never a parameter (`DECISIONS.md` 18, 22, 23): `CONNECTION`,
`VERBOSE`, `BOOTSTRAP_PORT`, `EVENT_SENDER`, `PROCESS_TABLE`, **`MOUSE_TAP_STATE`**, the three
window levels, the `__pending_*` / `__last_*` atomics.

`MOUSE_TAP_STATE` is what makes `src/mouse_handler.c` read oddly: `DECISIONS.md` 23 splits
`struct mouse_state` in two, and the half this file's tap, `mouse_state_init`,
`mouse_handler_begin` and `mouse_handler_end` touch — `handle`, `runloop_source`,
`consume_mouse_click`, `drag_detected`, `consumed_event`, `modifier`, `action1`, `action2`,
`drop_action` — is entirely the static. Only `mouse_window_info_populate` names a
`MouseDragState` field (`window`, `window_frame`), so only it carries the parameter by §2.1; the
four MAIN-thread functions carry nothing at all by `DECISIONS.md` 20.

Every state parameter is `&mut`, with no read-only variant (§2.1).

**Return types.** `bool` stays `bool` and observable sentinels stay integers
(`DECISIONS.md` 32) — `signal_type_from_string` still returns `SignalType::Unknown` rather than
an `Option`, and `mouse_determine_drop_action` still returns `MouseDropAction::None`. No function
in these files returns a record pointer or a list, so those clauses of the rule are vacuous here.
`FILE *rsp` becomes `response: &mut Response` and keeps its first position
(`GLOSSARY.md` §10.1, §10.4; `DECISIONS.md` 28).

---

## 2. `src/event_signal.c` — 10 rows

| C name | file:line | thread context | event-loop-owned state touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `event_signal_filter` | `src/event_signal.c:8` | event loop | — | `fn event_signal_filter(event_signal: &PendingSignal, signal: &Signal) -> bool` | private |
| `event_signal_flush` | `src/event_signal.c:60` | event loop (it forks; the child runs no manager code under `DECISIONS.md` 25) | `signal_event`, `signal_storage` | `pub(crate) fn event_signal_flush(signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], signal_storage: &mut Vec<PendingSignal>)` | `pub(crate)` |
| `event_signal_push` | `src/event_signal.c:99` | event loop | `signal_event`, `process_manager`, `display_manager`, `window_manager`, `space_manager`, `signal_storage` | `pub(crate) fn event_signal_push(signal_type: SignalType, context: SignalContext, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, signal_storage: &mut Vec<PendingSignal>)` | `pub(crate)` |
| `signal_type_from_string` | `src/event_signal.c:343` | event loop | — | `pub(crate) fn signal_type_from_string(string: &[u8]) -> SignalType` | `pub(crate)` |
| `event_signal_add` | `src/event_signal.c:352` | event loop | `signal_event` | `pub(crate) fn event_signal_add(signal_type: SignalType, signal: Signal, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT])` | `pub(crate)` |
| `event_signal_destroy` | `src/event_signal.c:358` | event loop | — | `fn drop(&mut self)` | `impl Drop for Signal` |
| `event_signal_remove_by_index` | `src/event_signal.c:368` | event loop | `signal_event` | `pub(crate) fn event_signal_remove_by_index(index: i32, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT]) -> bool` | `pub(crate)` |
| `event_signal_remove` | `src/event_signal.c:385` | event loop | `signal_event` | `pub(crate) fn event_signal_remove(label: &[u8], signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT]) -> bool` | `pub(crate)` |
| `event_signal_serialize` | `src/event_signal.c:400` | event loop | — | `fn event_signal_serialize(response: &mut Response, signal: &Signal, signal_type: SignalType, index: i32)` | private |
| `event_signal_list` | `src/event_signal.c:431` | event loop | `signal_event` | `pub(crate) fn event_signal_list(response: &mut Response, signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT])` | `pub(crate)` |

---

## 3. `src/rule.h` — 6 rows

The six `static inline` helpers become inherent methods on the two flag newtypes of
`GLOSSARY.md` §5.3 and §5.4, per `patterns/message-and-serialisation.md` §10.3. They take the
flag word, not the `struct rule *` / `struct rule_effects *` the C takes, because the C
parameter existed only to reach `->flags`.

| C name | file:line | thread context | event-loop-owned state touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `rule_check_flag` | `src/rule.h:59` | any | — | `pub(crate) fn contains(self, flag: RuleFlag) -> bool` | `pub(crate)` |
| `rule_clear_flag` | `src/rule.h:60` | — | — | *not translated, §6* | — |
| `rule_set_flag` | `src/rule.h:61` | any | — | `pub(crate) fn insert(&mut self, flag: RuleFlag)` | `pub(crate)` |
| `rule_effects_check_flag` | `src/rule.h:63` | any | — | `pub(crate) fn contains(self, flag: RuleEffectsFlag) -> bool` | `pub(crate)` |
| `rule_effects_clear_flag` | `src/rule.h:64` | event loop | — | `fn remove(&mut self, flag: RuleEffectsFlag)` | private to `crate::rule` |
| `rule_effects_set_flag` | `src/rule.h:65` | any | — | `pub(crate) fn insert(&mut self, flag: RuleEffectsFlag)` | `pub(crate)` |

`contains` / `insert` / `remove` are three methods on `RuleFlag` and three on `RuleEffectsFlag`;
the names collide across the two `impl` blocks only in the way any two inherent methods do.

---

## 4. `src/rule.c` — 10 rows

| C name | file:line | thread context | event-loop-owned state touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `rule_serialize` | `src/rule.c:4` | event loop | `display_manager` | `pub(crate) fn rule_serialize(response: &mut Response, rule: &Rule, index: i32, display_manager: &mut DisplayManager)` | `pub(crate)` |
| `rule_combine_effects` | `src/rule.c:63` | event loop | — | `pub(crate) fn rule_combine_effects(effects: &RuleEffects, result: &mut RuleEffects)` | `pub(crate)` |
| `rule_reapply_all` | `src/rule.c:113` | event loop | `process_manager`, `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state`, `mission_control_mode` | `pub(crate) fn rule_reapply_all(process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | `pub(crate)` |
| `rule_reapply_by_index` | `src/rule.c:131` | event loop | `process_manager`, `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state`, `mission_control_mode` | `pub(crate) fn rule_reapply_by_index(index: i32, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> bool` | `pub(crate)` |
| `rule_reapply_by_label` | `src/rule.c:145` | event loop | `process_manager`, `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state`, `mission_control_mode` | `pub(crate) fn rule_reapply_by_label(label: &[u8], process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> bool` | `pub(crate)` |
| `rule_apply` | `src/rule.c:159` | event loop | `process_manager`, `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state`, `mission_control_mode` | `pub(crate) fn rule_apply(rule: &Rule, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | `pub(crate)` |
| `rule_add` | `src/rule.c:175` | event loop | `window_manager` | `pub(crate) fn rule_add(rule: Rule, window_manager: &mut WindowManager)` | `pub(crate)` |
| `rule_remove_by_index` | `src/rule.c:181` | event loop | `window_manager` | `pub(crate) fn rule_remove_by_index(index: i32, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `rule_remove_by_label` | `src/rule.c:194` | event loop | `window_manager` | `pub(crate) fn rule_remove_by_label(label: &[u8], window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `rule_destroy` | `src/rule.c:207` | event loop | — | `fn drop(&mut self)` | `impl Drop for Rule` |

---

## 5. `src/mouse_handler.c` — 12 rows

| C name | file:line | thread context | event-loop-owned state touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `mouse_mod_from_cgflags` | `src/mouse_handler.c:5` | main run loop | none (`DECISIONS.md` 20) | `fn mouse_mod_from_cgflags(cgflags: u32) -> MouseMod` | private |
| `mouse_handler` | `src/mouse_handler.c:21` | main run loop | none (`DECISIONS.md` 20) | `unsafe extern "C-unwind" fn mouse_handler(proxy: CGEventTapProxy, event_type: CGEventType, event: NonNull<CGEvent>, context: *mut c_void) -> *mut CGEvent` | private |
| `mouse_window_info_populate` | `src/mouse_handler.c:90` | event loop | `window_manager`, `mouse_drag_state` | `pub(crate) fn mouse_window_info_populate(mouse_drag_state: &mut MouseDragState, info: &mut MouseWindowInfo, window_manager: &mut WindowManager)` | `pub(crate)` |
| `mouse_determine_drop_action` | `src/mouse_handler.c:108` | event loop | `window_manager`, `space_manager`, `mouse_drag_state` (declared, unread — §8.5) | `pub(crate) fn mouse_determine_drop_action(mouse_drag_state: &mut MouseDragState, source_space_id: SpaceId, source_node_id: NodeId, destination_window_id: WindowId, point: CGPoint, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> MouseDropAction` | `pub(crate)` |
| `mouse_drop_action_stack` | `src/mouse_handler.c:133` | event loop | `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state` | `pub(crate) fn mouse_drop_action_stack(window_manager: &mut WindowManager, space_manager: &mut SpaceManager, source_space_id: SpaceId, source_window_id: WindowId, destination_space_id: SpaceId, destination_window_id: WindowId, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | `pub(crate)` |
| `mouse_drop_action_swap` | `src/mouse_handler.c:153` | event loop | `window_manager`, `space_manager` | `pub(crate) fn mouse_drop_action_swap(window_manager: &mut WindowManager, space_manager: &mut SpaceManager, source_space_id: SpaceId, source_node_id: NodeId, source_window_id: WindowId, destination_space_id: SpaceId, destination_node_id: NodeId, destination_window_id: WindowId)` | `pub(crate)` |
| `mouse_drop_action_warp` | `src/mouse_handler.c:181` | event loop | `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state` | `pub(crate) fn mouse_drop_action_warp(window_manager: &mut WindowManager, space_manager: &mut SpaceManager, source_space_id: SpaceId, source_node_id: NodeId, source_window_id: WindowId, destination_space_id: SpaceId, destination_node_id: NodeId, destination_window_id: WindowId, split: WindowNodeSplit, child: WindowNodeChild, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | `pub(crate)` |
| `mouse_drop_no_target` | `src/mouse_handler.c:218` | event loop | `display_manager`, `window_manager`, `space_manager`, `mouse_drag_state` | `pub(crate) fn mouse_drop_no_target(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, source_space_id: SpaceId, destination_space_id: SpaceId, window_id: WindowId, node_id: NodeId, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | `pub(crate)` |
| `mouse_drop_try_adjust_bsp_grid` | `src/mouse_handler.c:232` | event loop | `display_manager`, `window_manager`, `space_manager` | `pub(crate) fn mouse_drop_try_adjust_bsp_grid(window_manager: &mut WindowManager, space_manager: &mut SpaceManager, space_id: SpaceId, window_id: WindowId, info: &MouseWindowInfo, display_manager: &mut DisplayManager)` | `pub(crate)` |
| `mouse_state_init` | `src/mouse_handler.c:266` | start-up only, main thread (`yabai.c:155`) | none — `MOUSE_TAP_STATE` only | `pub(crate) fn mouse_state_init()` | `pub(crate)` |
| `mouse_handler_begin` | `src/mouse_handler.c:274` | any — main at start-up (`yabai.c:307`) and event loop (`window_manager.c:224`, `:226`) | none — `MOUSE_TAP_STATE` only | `pub(crate) fn mouse_handler_begin(mask: u32) -> bool` | `pub(crate)` |
| `mouse_handler_end` | `src/mouse_handler.c:293` | event loop (`window_manager.c:221`) | none — `MOUSE_TAP_STATE` only | `pub(crate) fn mouse_handler_end()` | `pub(crate)` |

---

## 6. Dead functions — not translated

| C name | file:line | why |
| --- | --- | --- |
| `rule_clear_flag` | `src/rule.h:60` | defined, never called. `grep -n "rule_clear_flag(" src/` returns the definition and nothing else; the three `rule_*_flag` helpers are used 25 / 0 / 11 times. `DECISIONS.md` 5: "declarations with no definition or no caller". One `DEVIATIONS.md` line. `RuleEffectsFlag::remove` is **not** dead — `rule.c:70` and `:79` call `rule_effects_clear_flag` |

Every other function in these six files has a definition and a live caller. Nothing in them is
`#if 0`. The two `#pragma clang diagnostic` blocks around `mouse_handler`
(`src/mouse_handler.c:18-20`, `:88`) suppress warnings, not code, and have no Rust analogue.

---

## 7. Rust-only helpers and types this module adds

No C original, so §2.2 step 2 is vacuous for them: own parameters first, managers appended in the
step 3 order.

| Rust name | why it exists | definition | visibility |
| --- | --- | --- | --- |
| `SignalContext` | `event_signal_push`'s `void *context` is a tagged union whose tag is the `type` parameter (`event_signal.c:127`, `:138`, `:175`, `:189`, `:201`, `:216`, `:229`, `:243`, `:281`, `:295`, `:332`). `DECISIONS.md` 14 forbids the pointer and 39 confines `unsafe` to FFI, so the tag becomes an enum | `pub(crate) enum SignalContext { None, Application(ProcessId), Window(WindowId), Space(SpaceId), Display(DisplayId), MissionControl(MissionControlMode) }` | `pub(crate)` |
| `PreparedSignalCommand` | `DECISIONS.md` 25 and `THREADS.md` §9.1: the filter verdict, the `argv` and the `envp` are built in the parent before the fork, so nothing between `fork` and `execvp` allocates | owns its `CString`s and the two NUL-terminated `Vec<*const c_char>` | private to `crate::event_signal` |
| `event_signal_prepare_commands` | phase 1 of the same split | `fn event_signal_prepare_commands(signal_event: &[Vec<Signal>; SIGNAL_TYPE_COUNT], signal_storage: &[PendingSignal]) -> Vec<PreparedSignalCommand>` | private |

`Signal`, `PendingSignal`, `SignalProp`, `SignalType`, `SIGNAL_TYPE_COUNT`, `Rule`,
`RuleEffects`, `RuleFlag`, `RuleEffectsFlag`, `MouseTapState`, `MouseWindowInfo`, `MouseMod`,
`MouseMode`, `MouseDropAction` are all named by `GLOSSARY.md` §2 / §4 / §5 and are not new.

---

## 8. Derivations that are not a straight read of `state-access.tsv`

The tsv closure is textual: it does not model `DECISIONS.md` 14 (handles), it does not see a
manager that arrives as a C parameter, and it does not know that `struct mouse_state` split in
two. Eleven rows differ from it.

**8.1 — `mouse_window_info_populate` gains `window_manager`.** Its tsv row is
`explicit_manager_parameters=mouse_state`, `transitive_globals=-`. The body's only statement that
reads anything is `CGRect frame = ms->window->frame;` (`:92`). `ms->window` is
`MouseDragState::window_id: Option<WindowId>` (`GLOSSARY.md` §3.23), and §2.1 counts "the owning
manager of every handle that `f` resolves: … a `WindowId` through `WindowManager::window`". So
`Managers = { MouseDragState, WindowManager }`. `MouseDragState` is placed by step 2 at the C
position of `struct mouse_state *ms`; `WindowManager` is appended after `info`.

**8.2 — `mouse_determine_drop_action` gains `space_manager` and `window_manager`.** Same tsv
shape, same reason twice over: `src_node->window_count` (`:118`) resolves a `(SpaceId, NodeId)`
through `SpaceManager::view`, and `dst_window->frame` (`:110`) resolves a `WindowId` through
`WindowManager::window`. Appended in the step 3 order, `window_manager` (`yabai.c:30`) before
`space_manager` (`yabai.c:31`).

**8.3 — every `struct view *` parameter contributes `&mut SpaceManager`, but only once.**
§2.2 step 2: `struct view *` "contributes `&mut SpaceManager` at its own position, immediately
followed by the `SpaceId` that replaces the pointer". The five drop functions each take two
views; the second one finds `SpaceManager` already placed ("a manager that step 2 has already
placed is never placed again by step 3") and contributes only its `SpaceId`. That is why
`mouse_drop_action_swap` reads `(window_manager, space_manager, source_space_id, …,
destination_space_id, …)` and not two space managers. The tsv lists no `g_space_manager` for
`mouse_drop_action_swap` at all, because in C the views arrive as parameters.

**8.4 — a `struct window_node *` whose view is already a parameter contributes only a `NodeId`.**
`GLOSSARY.md` §1 and §3.1: "a node handle that crosses a `View` boundary is the pair
`(SpaceId, NodeId)`". In `mouse_drop_action_swap` / `_warp` / `mouse_drop_no_target` the node's
view is the adjacent `struct view *` parameter, so the `SpaceId` half is already in the list and
writing it twice would be a duplicate binding. `mouse_determine_drop_action` is the one row where
the node arrives with no view beside it, so there it is the full pair
(`source_space_id`, `source_node_id`) — the same shape `state-access/event_loop.md` passes and the
same shape §2.3 example (1) gives `insert_feedback_show`.

**8.5 — `mouse_determine_drop_action` keeps `mouse_drag_state` even though it reads nothing from
it.** Its only `ms->` access is `ms->drop_action` (`:119`), which `DECISIONS.md` 23 and
`GLOSSARY.md` §3.23 put on the `MOUSE_TAP_STATE` static. §2.2 step 2 names `struct mouse_state *`
explicitly as a declared manager type that "keeps its position like any other declared manager",
so the parameter stays and the body reads the static. Wave 1 binds it `_mouse_drag_state` if the
unused-parameter lint fires. See §10.4 for the three functions where `DECISIONS.md` 20 removes it
instead.

**8.6 — the three `MOUSE_TAP_STATE`-only functions lose their parameter entirely.**
`mouse_state_init` (`:266-272`) writes `modifier`, `action1`, `action2` and `drop_action`;
`mouse_handler_begin` (`:274-291`) and `mouse_handler_end` (`:293-303`) write `handle` and
`runloop_source`. All nine fields are `MouseTapState`, a static
(`patterns/state-and-ownership.md` §1.3). `mouse_state_init` additionally runs at `yabai.c:155`,
which is step 1 of §1.4 — **before** `EventLoopOwnedState` exists — so it could not take a
manager even if it wanted one. `DECISIONS.md` 20 covers `mouse_handler_begin` for its main-thread
caller. `state-access/window_manager.md` reaches the same conclusion from the other side for
`window_manager_set_focus_follows_mouse`: "It passes `&g_mouse_state` to `mouse_handler_end` /
`mouse_handler_begin`, which touch only `handle` and `runloop_source` … No `MouseDragState`."

**8.7 — `MouseDragState` propagates out of every node free.**
`patterns/state-and-ownership.md` §5.4 makes `view_free_node` clear
`MouseDragState::feedback_node`, which the C never does (`sweeps/globals-and-ownership.md` §9.1),
and `state-access/view.md` N12 therefore gives `view_remove_window_node`
(`view.c:621`, two `free`s at `:718-719`) a `mouse_drag_state: &mut MouseDragState`. The least
fixed point carries that to:

* `mouse_drop_action_warp` — calls `view_remove_window_node` directly (`:198`);
* `mouse_drop_action_stack` and `mouse_drop_no_target` — through
  `space_manager_untile_window` (`:135`, `:223`), which calls it at `space_manager.c:135`;
* `rule_reapply_all`, `rule_apply`, `rule_reapply_by_index`, `rule_reapply_by_label` — through
  `window_manager_apply_manage_rule_effects_to_window` →
  `window_manager_make_window_floating` (`window_manager.c:110`, `:114`) →
  `space_manager_untile_window` (`window_manager.c:2198`).

`mouse_drop_action_swap` frees nothing (`window_node_swap_window_list` and
`window_node_capture_windows` only read and reorder) and `mouse_drop_try_adjust_bsp_grid` frees
nothing (`window_manager_resize_window_relative` adjusts two `ratio` fields and calls
`view_update` / `view_flush`), so neither takes it. See §10.6 — three sibling documents stop the
propagation one call earlier and must be reconciled.

**8.8 — the `rule_*` rows gain `process_manager`.** The tsv lists `g_process_manager` in the
transitive column of all four, and it is the `ProcessManager` half, not `PROCESS_TABLE`: the
reach is `rule_apply` → `window_manager_apply_rule_effects_to_window` (`window_manager.c:119`) →
`window_manager_send_window_to_space` (`:2092`) → `_SLPSSetFrontProcessWithOptions(&g_process_manager.finder_psn, …)`
(`:2104`). `THREADS.md` §1.4 puts `finder_psn` in the event-loop-owned half, so it is a parameter.
`state-access/window_manager.md` carries the same `process_manager` on
`window_manager_apply_rule_effects_to_window` and `window_manager_apply_rules_to_window`.

**8.9 — `rule_serialize` gains `display_manager` and does not gain `space_manager`.** Its two
non-trivial callees split: `display_manager_display_id_arrangement` (`display_manager.c:165`)
reads `g_display_manager.order` at `:178`, so it takes `&mut DisplayManager`;
`space_manager_mission_control_index` (`space_manager.c:491`) reads only `g_connection`, a static,
so it takes nothing. `state-access/space.md` gives it
`pub(crate) fn space_manager_mission_control_index(space_id: SpaceId) -> i32`.

**8.10 — `event_signal_push`'s six members are whole-function, not per-`switch`-arm.**
`Managers(f)` is defined over the function, so every arm's state travels to every call site:
`g_signal_event` (`:101`), `g_signal_storage` (`:107-108`),
`g_process_manager.switch_event_time` / `.front_pid` / `.last_front_pid` (`:156-160`, `:170-172`,
`:184`), `g_window_manager.focused_window_id` (`:210`, `:226`),
`g_space_manager.current_space_id` / `.last_space_id` (`:252-253`),
`g_display_manager.current_display_id` / `.last_display_id` (`:304-305`). It agrees with the tsv
on all six and with `state-access/event_loop.md` §5 item 4, which derives the identical set for
all 35 handler call sites.

**8.11 — `event_signal_flush` and `event_signal_filter` run only on the event loop in Rust.**
In C both also execute inside the forked child (`event_signal.c:64-96`, `:81`); `DECISIONS.md` 25
moves the filter verdict, the `debug!` line, the `argv` and the environment into the parent
(`THREADS.md` §9.1), leaving the child with `fork`, a store into `environ`, `execvp` and `_exit`.
`event_signal_filter`'s signature is unaffected either way — it takes no state.

---

## 9. Cross-module calls this module makes

Shapes this module passes. The manager tail is binding because it comes out of the same fixed
point; the owning wave-1 unit fixes everything else. Rows marked **(!)** are ones where the
owning `state-access/*.md` currently prints a shorter tail; §10.6.

| callee | owning file | shape called with |
| --- | --- | --- |
| `regex_match` | `src/misc/regex.rs` (`misc/macros.h:26`) | `(regex: Option<&PosixRegex>, subject: &CStr) -> RegexMatch` — the `bool valid` argument is gone into the `Option` |
| `string_equals` | `src/misc/helpers.h:254` | `(first: Option<&str>, second: Option<&str>) -> bool` — NULL on either side never matches |
| `json_bool` / `json_optional_bool` | `src/misc/helpers.h:220`, `:225` | `(value) -> &'static str` |
| `json_escape` (`ts_string_escape`) | `src/misc/helpers.h:259` | `(text: &str) -> Option<String>` — `None` means nothing needed escaping |
| `display_manager_display_id_arrangement` | `src/display_manager.c:165` | `(display_id: DisplayId, display_manager: &mut DisplayManager) -> i32` |
| `space_manager_mission_control_index` | `src/space_manager.c:491` | `(space_id: SpaceId) -> i32` — no manager |
| `window_title_ts` / `window_role_ts` / `window_subrole_ts` | `src/window.c:724`, `:1001`, `:1022` | `(window_id: WindowId, window_manager: &mut WindowManager) -> Option<String>` |
| `window_manager_is_window_eligible` | `src/window_manager.c:19` | `(window_id: WindowId, window_manager: &mut WindowManager) -> bool` |
| `window_manager_rule_matches_window` | `src/window_manager.c:91` | `(rule: &Rule, window_id: WindowId, window_title: Option<&str>, window_role: Option<&str>, window_subrole: Option<&str>, window_manager: &mut WindowManager) -> bool` |
| `window_manager_apply_manage_rules_to_window` **(!)** | `src/window_manager.c:171` | `(space_manager, window_manager, window_id, window_title, window_role, window_subrole, one_shot_rules, display_manager, mouse_drag_state)` |
| `window_manager_apply_rules_to_window` **(!)** | `src/window_manager.c:195` | `(space_manager, window_manager, window_id, window_title, window_role, window_subrole, one_shot_rules, process_manager, display_manager, mouse_drag_state)` |
| `window_manager_apply_manage_rule_effects_to_window` **(!)** | `src/window_manager.c:108` | `(space_manager, window_manager, window_id, effects: &RuleEffects, display_manager, mouse_drag_state)` |
| `window_manager_apply_rule_effects_to_window` **(!)** | `src/window_manager.c:119` | `(space_manager, window_manager, window_id, effects: &RuleEffects, process_manager, display_manager, mouse_drag_state)` |
| `space_manager_untile_window` **(!)** | `src/space_manager.c:130` | `(space_manager, space_id, window_id, display_manager, window_manager, mouse_drag_state)` |
| `space_manager_tile_window_on_space` | `src/space_manager.c:462` | `(space_manager, window_id, space_id, display_manager, window_manager) -> SpaceId` |
| `window_manager_remove_managed_window` | `src/window_manager.c:288` | `(window_manager, window_id)` |
| `window_manager_add_managed_window` | `src/window_manager.c:293` | `(window_manager, window_id, space_manager, space_id)` |
| `window_manager_purify_window` | `src/window_manager.c:877` | `(window_manager, window_id)` |
| `window_manager_find_window` | `src/window_manager.c:1394` | `(window_manager, window_id) -> Option<WindowId>` |
| `window_manager_adjust_layer` | `src/window_manager.c:820` | `(window_id, layer: i32, window_manager)` |
| `window_manager_animate_window` | `src/window_manager.c:718` | `(capture: WindowCapture, window_manager)` |
| `window_manager_animate_window_list` | `src/window_manager.c:705` | `(window_list: &[WindowCapture], window_manager)` — the `ts_buf_len` argument collapses into the slice (§2.2 example 3) |
| `window_manager_resize_window_relative` | `src/window_manager.c:368` | `(window_manager, window_id, direction: i32, delta_x: f32, delta_y: f32, animate: bool, display_manager, space_manager) -> WindowOpError` |
| `view_find_window_node` | `src/view.c:612` | `(space_manager, space_id, window_id) -> Option<NodeId>` |
| `view_remove_window_node` | `src/view.c:621` | `(space_manager, space_id, window_id, display_manager, window_manager, mouse_drag_state) -> Option<NodeId>` |
| `view_add_window_node_with_insertion_point` | `src/view.c:751` | `(space_manager, space_id, window_id, insertion_point: WindowId, display_manager, window_manager) -> Option<NodeId>` |
| `view_stack_window_node` | `src/view.c:730` | `(space_id, node_id, window_id, space_manager)` |
| `window_node_contains_window` | `src/view.c:381` | `(space_id, node_id, window_id, space_manager) -> bool` |
| `window_node_swap_window_list` | `src/view.c:399` | `(a_space_id, a_node_id, b_space_id, b_node_id, space_manager)` |
| `window_node_capture_windows` | `src/view.c:358` | `(space_id, node_id, window_list: &mut Vec<WindowCapture>, window_manager, space_manager)` — the out-parameter is kept, per `state-access/view.md` N5 |
| `window_node_flush` | `src/view.c:374` | `(space_id, node_id, window_manager, space_manager)` |
| `scripting_addition_order_window` | `src/sa.h` | `(window_id: WindowId, order: i32, relative_window_id: WindowId) -> bool` — no manager |
| `event_loop_post` | `src/event_loop.c:1684` | `(event: Event)` — `EVENT_SENDER` static; `mouse_handler` posts `Event::MouseDown { event, modifier }`, `MouseUp`, `MouseDragged`, `MouseMoved { event, modifier }` with `SendCFRetained<CGEvent>` payloads (`THREADS.md` §3.1) |
| `read_os_timer` | `src/misc/timer.h` | `() -> u64` |
| `triangle_contains_point` | `src/misc/helpers.h:564` | `(triangle: &[CGPoint; 3], point: CGPoint) -> bool` — cross products in `f32` |
| `GetCurrentEventTime` | Carbon | `() -> f64`; the threshold at `event_signal.c:157` is `f64::from(0.05_f32)`, never `0.05_f64` |
| CoreGraphics / CoreFoundation | `src/ffi/` | `CGEventTapCreate`, `CGEventTapEnable`, `CGEventTapIsEnabled`, `CGEventTapPostEvent`, `CGEventGetFlags`, `CGEventGetIntegerValueField`, `CGRectContainsPoint`, `CFMachPortInvalidate`, `CFMachPortCreateRunLoopSource`, `CFRunLoopGetMain`, `CFRunLoopAddSource`, `CFRunLoopRemoveSource`, `CFRetain`, `CFRelease` |
| `libc` | — | `fork`, `execvp`, `_exit`, `_NSGetEnviron`, `regcomp` / `regexec` / `regfree` behind `PosixRegex` |

Callers of this module, for the record: `src/event_loop.c` (`event_signal_push` ×35,
`event_signal_flush`, the six `mouse_drop_*` / `mouse_*` entry points, `rule_destroy` at `:571`),
`src/message.c` (`signal_type_from_string`, `event_signal_add`, `event_signal_destroy`,
`event_signal_remove*`, `event_signal_list`, `rule_add`, `rule_reapply_*`, `rule_apply`,
`rule_remove_*`, `rule_destroy`), `src/window_manager.c` (`rule_serialize`,
`rule_combine_effects`, `rule_destroy`, `mouse_handler_begin`, `mouse_handler_end`) and
`src/yabai.c` (`mouse_state_init`, `mouse_handler_begin`).

---

## 10. Judgement calls

Every place the mechanical rule was ambiguous, and what this unit chose. Where a phase-1 document
or a sibling wave-0b document says otherwise, the losing spelling is named so the disagreement is
visible rather than silent.

1. **`signal_event` and `signal_storage` are appended like managers, in the `yabai.c:27-35`
   positions.** `patterns/state-and-ownership.md` §2.2 step 3 enumerates five managers and stops,
   because §2.1 defines `Managers(f)` over managers. `g_signal_event` (`yabai.c:27`) and
   `g_signal_storage` (`yabai.c:32`) are `EventLoopOwnedState` fields (§1.1, §1.2) and
   `DECISIONS.md` 13 forbids reaching event-loop-owned state through a global, so they must be
   parameters; step 3's own words are "in the `src/yabai.c:27-35` order", which places
   `signal_event` first and `signal_storage` between `space_manager` (`:31`) and
   `mouse_drag_state` (`:33`). Without this, `src/event_signal.c` has no translation at all.
   `state-access/event_loop.md` §8 item 1 makes the identical call, and `state-access/space.md`
   and `window_manager.md` use the same table, so the four documents agree.

2. **`event_signal_flush` takes the two fields, not `&mut EventLoopOwnedState`.**
   `THREADS.md` §9.2 writes it as
   `fn event_signal_flush(event_loop_owned_state: &mut EventLoopOwnedState)`, and
   `state-access/event_loop.md` §6 repeats that shape at the call site.
   `patterns/state-and-ownership.md` §1.1 says "**That field order is not a parameter order.** No
   signature is derived from this struct", and `DECISIONS.md` 13 — which settles every
   disagreement — says functions take "the managers they touch". Rejected: the whole-struct
   parameter. The call site in `event_loop_run` owns the struct and becomes
   `event_signal_flush(&mut state.signal_event, &mut state.signal_storage)`, which is a one-line
   change on the caller's side.

3. **`response: &mut Response`, not `&mut dyn std::io::Write`.** `GLOSSARY.md` §10.1 and §10.4
   and `DECISIONS.md` 28 name the `Response` type of
   `patterns/message-and-serialisation.md` §7.1; §2.2's `dyn Write` phrasing predates it.
   `state-access/window_manager.md` judgement call 3 is the same decision.

4. **The declared `struct mouse_state *` survives only where a `MouseDragState` field is named.**
   §2.2 step 2 says the parameter keeps its position; §2.1 says `MouseDragState` is in
   `Direct(f)` only "if `f` names any field of it". Read together: it is a parameter when the
   function names a `MouseDragState` field, placed at the C position. So
   `mouse_window_info_populate` keeps it (`window`, `window_frame`),
   `mouse_determine_drop_action` keeps it (§8.5 — §2.2 step 2 names the type explicitly and it is
   an event-loop function), and `mouse_state_init` / `mouse_handler_begin` / `mouse_handler_end`
   lose it, since every field they touch is `MOUSE_TAP_STATE` and `DECISIONS.md` 20 forbids a
   manager on their main-thread callers anyway. Rejected: an unconditional
   `&mut MouseDragState` on all five, which would hand `mouse_state_init` a parameter that does
   not yet exist when `yabai.c:155` runs.

5. **`mouse_window_info_populate` keeps its out-parameter; `mouse_determine_drop_action`'s
   destination window is a bare `WindowId`.** §2.2's three mechanical consequences cover an
   `int *count` paired with a returned buffer, a `(pointer, count)` pair, and `FILE *rsp`. A
   `struct mouse_window_info *` out-parameter is none of them, and every field is written before
   any is read, so `DECISIONS.md` 4's uninitialised-read clause does not bite either; it stays
   `info: &mut MouseWindowInfo`. `state-access/view.md` N5 keeps
   `window_node_capture_windows`'s out-parameter for the same reason, so the two agree.
   `files/mouse-workspace-sa.md` §2.3 recommends returning `MouseWindowInfo` by value; rejected,
   it is a recommendation, not a rule. For `dst_window`: `state-access/event_loop.md` §6 passes
   `destination_window_id: Option<WindowId>`, but `mouse_handler.c:110` dereferences the pointer
   unconditionally and `event_loop.c:1188-1189` guarantees it is non-NULL (`b_node` is `NULL`
   whenever `window` is), so `DECISIONS.md` 32's "nullable pointers become `Option`" does not
   apply. `WindowId`, and the caller unwraps.

6. **`MouseDragState` propagates through every node free, which three sibling documents stop
   short of.** §8.7 gives the chain. `state-access/view.md` N12 puts
   `mouse_drag_state: &mut MouseDragState` on `view_remove_window_node`, as
   `patterns/state-and-ownership.md` §5.4 requires; `state-access/space.md`'s
   `space_manager_untile_window`, `state-access/window_manager.md`'s
   `window_manager_make_window_floating` / `window_manager_apply_*rule*_to_window` and
   `state-access/event_loop.md` §6's `space_manager_untile_window` /
   `view_remove_window_node` rows all omit it. §2.1 is explicit that "a later disagreement between
   a caller and a callee means the fixed point was computed wrong, not that a signature should be
   patched locally", and §5.4 is the code, so this unit propagates. The rows it affects here are
   `rule_reapply_all`, `rule_reapply_by_index`, `rule_reapply_by_label`, `rule_apply`,
   `mouse_drop_action_stack`, `mouse_drop_action_warp` and `mouse_drop_no_target`.
   **The cross-module pass upheld this and propagated it.** §5.4's clearing of `feedback_node`
   stands — the arena of §5.1 recycles a freed `NodeId` through `View::free_node_ids`, so a stale
   `MouseDragState::feedback_node` would not merely dangle as it does in C, it would silently name
   a *different* live node. The twenty-nine sibling rows that were missing the parameter gained
   it: six `EVENT_HANDLER` rows in `state-access/event_loop.md`, three `handle_domain_*` rows in
   `state-access/message.md`, four in `state-access/space.md` and sixteen in
   `state-access/window_manager.md`. Nothing in this document changed.

7. **The four `rule.c` reapply rows gained `mission_control_mode: &mut MissionControlMode`,
   appended last.** Added by the cross-module pass. `state-access.tsv` lists
   `g_mission_control_mode` in the transitive set of `rule_apply`, `rule_reapply_all`,
   `rule_reapply_by_index` and `rule_reapply_by_label` — the chain is
   `window_manager_apply_rule_effects_to_window` → `space_manager_focus_space`
   (`window_manager.c:127`) → `mission_control_is_active` (`space_manager.c:987`).
   `TRANSLATION_PLAN.md` §3.2 puts `g_mission_control_mode` on the list of event-loop-owned
   globals step 3 scans for, and `DECISIONS.md` 13 forbids reaching it through a global. It sorts
   after `mouse_drag_state` because `yabai.c:37` follows `:33` and because that is the
   `EventLoopOwnedState` field order (`patterns/state-and-ownership.md` §1.1).

8. **`label` and `str` parameters are `&[u8]`, not `&str`.** Changed by the cross-module pass on
   `rule_reapply_by_label`, `rule_remove_by_label`, `event_signal_remove` and
   `signal_type_from_string`. Every value that reaches them comes out of the message buffer —
   `message.c:2832`, `:2851`, `:2928`, `:2966` all pass `value.string_value` — and
   `patterns/message-and-serialisation.md` §3 names those exact sites, typing the replacement
   `c_string_at(message_bytes, value.token.start)` as `&[u8]`. The two callers that pass a stored
   `String` (`rule.c:177`, `event_signal.c:354`) pass `.as_bytes()`.

9. **`rule_apply` takes `&Rule`, not a `rule_index`.** §3 enumerates the five pointer kinds that
   become handles and `struct rule *` is not one, so it stays a borrow. Two of its three callers
   (`rule_reapply_by_index` `:136`, `rule_reapply_by_label` `:150`) pass
   `&g_window_manager.rules[i]` while `&mut WindowManager` is live, which does not borrow-check:
   the body must `std::mem::take` the `Vec<Rule>` for the duration and put it back, exactly as
   `state-access/window_manager.md` judgement call 5 prescribes for
   `window_manager_apply_manage_rules_to_window` (`:174`) and
   `window_manager_apply_rules_to_window` (`:198`). `Rule` is deliberately neither `Copy` nor
   `Clone` (`patterns/message-and-serialisation.md` §10.5). The alternative `rule_index: usize`
   must be taken for all three at once or for none; this unit takes none.

10. **`rule_add` is `(rule, window_manager)`, not `(window_manager, rule)`.**
   `patterns/message-and-serialisation.md` §10.5 writes
   `pub fn rule_add(window_manager: &mut WindowManager, rule: Rule)`. §2.2 step 3 appends the
   manager block after the whole C parameter list, and `rule_add(struct rule *rule)` declares no
   manager, so the manager goes last. `DECISIONS.md` 13's "in the C parameter order, with any
   manager C reached through a global appended" is binding. The same reasoning fixes
   `event_signal_add(signal_type, signal, signal_event)`.

11. **`rule_destroy` and `event_signal_destroy` are `Drop` impls, not functions.**
   `patterns/message-and-serialisation.md` §10.3 and §10.4 and
   `patterns/state-and-ownership.md` §3.2 all say so: the four/two `regex_t`s become
   `Option<PosixRegex>` whose `Drop` calls `regfree`, and the owned `char *`s become
   `Option<String>`. The C functions neither clear the flags nor NULL the pointers, so calling
   either twice is a double free; `Drop` makes that unrepresentable. They keep a row each in §2
   and §4 because they have a C definition and live callers — they are not dead code — but the
   Rust "name" is `drop` and the call sites at `message.c:2823`, `:2837`, `:2957`,
   `event_loop.c:571` and `window_manager.c:1568` become a `swap_remove` or nothing at all.

12. **`mouse_mod_from_cgflags` takes `u32` and returns `MouseMod`.** The C parameter is
    `uint32_t` and the call sites truncate `CGEventGetFlags`'s `CGEventFlags` (`u64`) implicitly;
    `DECISIONS.md` 30 turns that into an explicit `as u32` at the call site rather than widening
    the parameter, and `DECISIONS.md` 3 requires the truncation to survive.
    `files/mouse-workspace-sa.md` §2.3 recommends `u64`; rejected, it is a behaviour change by
    its own admission. The return is the bit-set the function OR-s together, which
    `GLOSSARY.md` §2.2 and §5.6 spell `MouseMod(pub u8)`; `THREADS.md` §3.1's `MouseModifier` is
    the rejected spelling, as `state-access/event_loop.md` §8 item 6 also records.
    `mouse_handler_begin`'s `mask` stays `u32` because `GLOSSARY.md` §7.2 declares
    `MOUSE_EVENT_MASK` and `MOUSE_EVENT_MASK_FFM` as `u32` consts.

13. **`mouse_handler`'s second parameter is `event_type`, not `r#type`.**
    `patterns/ffi-objc-and-os.md` §19.1 gives the signature verbatim, with `r#type: CGEventType`;
    `GLOSSARY.md` §11 says "**No `r#type` anywhere**" and names this exact site,
    `MOUSE_HANDLER(name)` (`mouse_handler.h:21`), as becoming `event_type`. The glossary is the
    binding spelling authority (`DECISIONS.md` 37). Everything else in that signature —
    `unsafe extern "C-unwind"`, `NonNull<CGEvent>`, `-> *mut CGEvent`, the `*mut c_void` refcon
    that Rust passes as `core::ptr::null_mut()` — is taken unchanged. It stays private: the tsv
    shows no caller outside the file, and `CGEventTapCreate` names it from within
    `mouse_handler_begin`. `patterns/ffi-objc-and-os.md` writes `pub`; the brief's tsv-driven
    visibility rule wins, as it does in `state-access/event_loop.md` §8 item 11.

14. **`SignalContext` is invented, because `void *context` cannot survive.**
    `event_signal_push`'s context is a tagged union with eleven decode sites and six payload
    kinds (`struct application *`, `struct window *`, a space id, a display id, a
    `mission_control_mode`, and `NULL`). `DECISIONS.md` 14 forbids the pointer, 39 confines
    `unsafe` to FFI and refcon casts, and no phase-1 document names a type for it —
    `state-access/event_loop.md` §6 explicitly leaves it to this unit ("the `context` payload type
    is the event-signal unit's to name"). §7 defines it. The alternative, eleven `event_signal_push_*`
    functions, would break `DECISIONS.md` 2's one-C-function-one-Rust-function rule.

15. **The six `rule.h` inline helpers become methods on the flag newtypes, which fixes
    `Rule::flags`' type.** `patterns/message-and-serialisation.md` §10.3 says they "become
    `contains` / `insert` / `remove` methods on the newtypes". `GLOSSARY.md` §3.19 and §3.20
    describe `Rule::flags` and `RuleEffects::flags` as "`u16`, tested with `RuleFlag`", which a
    method cannot hang off. This unit reads §3.19's "tested with `RuleFlag`" as naming the
    newtype the field holds — `flags: RuleFlag` and `flags: RuleEffectsFlag`, each a
    `pub(crate) struct Name(pub u16)` per §5's shape line. The serialised
    `"flags":"0x%08x"` of `rule.c:60` is then
    `((effects.flags.0 as u32) << 16) | (rule.flags.0 as u32)`, byte-identical.
    `patterns/message-and-serialisation.md` §10.3's plural `RuleFlags` / `RuleEffectsFlags` is
    the rejected spelling; `GLOSSARY.md` §2.2 and §13 fix the singular.

16. **`rule_effects_clear_flag` is translated and `rule_clear_flag` is not.** Both are
    `static inline` one-liners in the same header, and the mechanical test is the same for both:
    `DECISIONS.md` 5 drops a definition with no caller. `rule_effects_clear_flag` has two
    (`rule.c:70`, `:79`); `rule_clear_flag` has none. The asymmetry is real, not a slip — and it
    means `RuleFlag` carries `contains` and `insert` but no `remove`, while `RuleEffectsFlag`
    carries all three.

17. **`mouse_determine_drop_action`'s manager tail is `window_manager, space_manager`.**
    `state-access/event_loop.md` §6 passes `space_manager, window_manager` on that one row; §2.2
    step 3's order is `window_manager` (`yabai.c:30`) then `space_manager` (`yabai.c:31`), which
    the same document uses correctly everywhere else — `window_node_flush`,
    `insert_feedback_show`, `insert_feedback_destroy`. The declaration order in `yabai.c` is "the
    only tie-break there is".
