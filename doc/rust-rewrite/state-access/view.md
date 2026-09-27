# `src/view.h` + `src/view.c` — final Rust signatures

Wave 1 pastes the **Rust signature** column verbatim. Nothing in this file may be re-derived
locally; a disagreement between a caller in another module and a row here means the fixed point
of `patterns/state-and-ownership.md` §2.1 was computed wrong, not that a signature should be
patched.

Module: `crate::view`. Every type name in the signatures comes from `GLOSSARY.md` §2, §3.9-3.17
and §5.5: `Area` (fields `x`, `y`, `width`, `height`, all `f32`), `WindowNode`, `View`,
`WindowCapture`, `BalanceNode`, `ViewType`, `WindowNodeSplit`, `WindowNodeChild`,
`WindowInsertionPoint`, `FeedbackWindow`, `ViewFlag`. Handles come from `GLOSSARY.md` §1:
`SpaceId`, `NodeId`, `WindowId`, `DisplayId`, plus `ROOT_NODE_ID`.

## How each row was derived

`patterns/state-and-ownership.md` §2.2, in its three steps, without exception:

1. The C parameter list in C order, each pointer rewritten to its handle — `struct window *` →
   `WindowId`, `struct window_node *` → `(SpaceId, NodeId)`, `struct view *` → `SpaceId`,
   `FILE *rsp` → `response: &mut Response` in first position.
2. A manager the C declares keeps its C position. `view.c` declares none, but `struct view *`
   is the shape-changing case of step 2: it contributes `&mut SpaceManager` at its own position,
   immediately followed by the `SpaceId` that replaces the pointer. Nineteen rows below are that
   case.
3. Every remaining manager of `Managers(f)` is appended after the whole C parameter list, in the
   `src/yabai.c:27-35` order: `display_manager` (`:29`), `window_manager` (`:30`),
   `space_manager` (`:31`), `mouse_drag_state` (`:33`). `process_manager` (`:28`) is reached by
   nothing in this file.

`Managers(f)` per §2.1 is the transitive closure of the globals in `state-access.tsv` **plus the
owning manager of every handle the function resolves**. The tsv extraction is textual and knows
nothing about handles, so `SpaceManager` is added to every function that resolves a
`(SpaceId, NodeId)` or a `SpaceId` — which is almost every function in this file, because every
C `struct window_node *` and `struct view *` parameter is resolved, not merely passed along.
`g_connection`, `g_bs_port` and `g_temp_storage` contribute nothing: the first two are the
`OnceLock` statics of §1.2 (`DECISIONS.md` 18) and the third is gone (`DECISIONS.md` 17).

Every manager parameter is `&mut`, with no read-only variant (§2.1).

## Thread context

`files/view-and-tests.md` §0.1, re-checked against every caller: **`view.c` is single-threaded,
event-loop-pthread only.** Every caller sits in an `EVENT_HANDLER_*` body, in a `message.c`
command handler (reached only from `EVENT_HANDLER_DAEMON_MESSAGE`, because the message-loop
pthread does nothing but `accept()` + `event_loop_post`), or in a `window_manager.c` /
`space_manager.c` / `mouse_handler.c` function that is itself reached only from those two.

The one addition: `view_create` is also called from `space_manager_begin`
(`space_manager.c:1225`), which `patterns/state-and-ownership.md` §1.4 step 3 runs **on the main
thread against `&mut state`**, before the event-loop thread is spawned. That is not a breach of
`DECISIONS.md` 20 — it is the construction phase, where main *is* the owner. The nine functions
`view_create` reaches there are marked `event loop; also main at start-up`. The three animation
structs *declared* in `view.h` (`WindowProxy`, `WindowAnimation`, `AnimationContext`) are used
only from `window_manager.c` and are not this file's functions.

The pure geometry helpers (`area_from_cgrect`, `area_max_point`, `area_make_pair`,
`area_is_in_direction`, `area_distance_in_direction`, `balance_node_add`) touch no state at all
and are marked `any`; `DECISIONS.md` 40 transposes `tests/src/area.c` against three of them
inside this module.

---

## Signatures, in C source order

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `insert_feedback_show` | `src/view.c:8` | event loop | `WindowManager`, `SpaceManager` | `pub(crate) fn insert_feedback_show(space_id: SpaceId, node_id: NodeId, window_manager: &mut WindowManager, space_manager: &mut SpaceManager)` | `pub(crate)` |
| `insert_feedback_destroy` | `src/view.c:105` | event loop | `WindowManager`, `SpaceManager` | `pub(crate) fn insert_feedback_destroy(space_id: SpaceId, node_id: NodeId, window_manager: &mut WindowManager, space_manager: &mut SpaceManager)` | `pub(crate)` |
| `area_from_cgrect` | `src/view.c:121` | any | — | `pub(crate) fn area_from_cgrect(rect: CGRect) -> Area` | `pub(crate)` |
| `area_max_point` | `src/view.c:126` | any | — | `pub(crate) fn area_max_point(area: Area) -> CGPoint` | `pub(crate)` |
| `window_node_get_child` | `src/view.c:131` | event loop | `SpaceManager` | `fn window_node_get_child(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> WindowNodeChild` | private |
| `window_node_get_split` | `src/view.c:136` | event loop; also main at start-up | `SpaceManager` | `pub(crate) fn window_node_get_split(space_manager: &mut SpaceManager, space_id: SpaceId, node_id: NodeId) -> WindowNodeSplit` | `pub(crate)` |
| `window_node_get_ratio` | `src/view.c:151` | event loop; also main at start-up | `SpaceManager` | `pub(crate) fn window_node_get_ratio(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> f32` | `pub(crate)` |
| `window_node_get_gap` | `src/view.c:156` | event loop; also main at start-up | `SpaceManager` | `pub(crate) fn window_node_get_gap(space_manager: &mut SpaceManager, space_id: SpaceId) -> i32` | `pub(crate)` |
| `area_make_pair` | `src/view.c:161` | any | — | `pub(crate) fn area_make_pair(split: WindowNodeSplit, gap: i32, ratio: f32, parent_area: Area) -> (Area, Area)` | `pub(crate)` |
| `area_make_pair_for_node` | `src/view.c:186` | event loop; also main at start-up | `SpaceManager` | `fn area_make_pair_for_node(space_manager: &mut SpaceManager, space_id: SpaceId, node_id: NodeId)` | private |
| `window_node_is_occupied` | `src/view.c:198` | event loop | `SpaceManager` | `fn window_node_is_occupied(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> bool` | private |
| `window_node_is_intermediate` | `src/view.c:203` | event loop | `SpaceManager` | `pub(crate) fn window_node_is_intermediate(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> bool` | `pub(crate)` |
| `window_node_is_leaf` | `src/view.c:208` | event loop; also main at start-up | `SpaceManager` | `pub(crate) fn window_node_is_leaf(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> bool` | `pub(crate)` |
| `window_node_is_left_child` | `src/view.c:213` | event loop | `SpaceManager` | `pub(crate) fn window_node_is_left_child(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> bool` | `pub(crate)` |
| `window_node_is_right_child` | `src/view.c:218` | event loop | `SpaceManager` | `fn window_node_is_right_child(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> bool` | private |
| `window_node_equalize` | `src/view.c:223` | event loop | `SpaceManager` | `pub(crate) fn window_node_equalize(space_id: SpaceId, node_id: NodeId, axis_flag: u32, space_manager: &mut SpaceManager)` | `pub(crate)` |
| `balance_node_add` | `src/view.c:237` | any | — | `fn balance_node_add(first: BalanceNode, second: BalanceNode) -> BalanceNode` | private |
| `window_node_balance` | `src/view.c:242` | event loop | `SpaceManager` | `pub(crate) fn window_node_balance(space_id: SpaceId, node_id: NodeId, axis_flag: u32, space_manager: &mut SpaceManager) -> BalanceNode` | `pub(crate)` |
| `window_node_split` | `src/view.c:277` | event loop | `SpaceManager` | `fn window_node_split(space_manager: &mut SpaceManager, space_id: SpaceId, node_id: NodeId, window_id: WindowId)` | private |
| `window_node_update` | `src/view.c:324` | event loop; also main at start-up | `SpaceManager`, `WindowManager` | `pub(crate) fn window_node_update(space_manager: &mut SpaceManager, space_id: SpaceId, node_id: NodeId, window_manager: &mut WindowManager)` | `pub(crate)` |
| `window_node_destroy` | `src/view.c:335` | event loop | `WindowManager`, `SpaceManager`, `MouseDragState` | `fn window_node_destroy(space_id: SpaceId, node_id: NodeId, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)` | private |
| `window_node_clear_zoom` | `src/view.c:348` | event loop | `SpaceManager` | `fn window_node_clear_zoom(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager)` | private |
| `window_node_capture_windows` | `src/view.c:358` | event loop | `WindowManager`, `SpaceManager` | `pub(crate) fn window_node_capture_windows(space_id: SpaceId, node_id: NodeId, window_list: &mut Vec<WindowCapture>, window_manager: &mut WindowManager, space_manager: &mut SpaceManager)` | `pub(crate)` |
| `window_node_flush` | `src/view.c:374` | event loop | `WindowManager`, `SpaceManager` | `pub(crate) fn window_node_flush(space_id: SpaceId, node_id: NodeId, window_manager: &mut WindowManager, space_manager: &mut SpaceManager)` | `pub(crate)` |
| `window_node_contains_window` | `src/view.c:381` | event loop | `SpaceManager` | `pub(crate) fn window_node_contains_window(space_id: SpaceId, node_id: NodeId, window_id: WindowId, space_manager: &mut SpaceManager) -> bool` | `pub(crate)` |
| `window_node_index_of_window` | `src/view.c:390` | event loop | `SpaceManager` | `pub(crate) fn window_node_index_of_window(space_id: SpaceId, node_id: NodeId, window_id: WindowId, space_manager: &mut SpaceManager) -> i32` | `pub(crate)` |
| `window_node_swap_window_list` | `src/view.c:399` | event loop | `SpaceManager` | `pub(crate) fn window_node_swap_window_list(a_space_id: SpaceId, a_node_id: NodeId, b_space_id: SpaceId, b_node_id: NodeId, space_manager: &mut SpaceManager)` | `pub(crate)` |
| `window_node_find_first_leaf` | `src/view.c:421` | event loop | `SpaceManager` | `pub(crate) fn window_node_find_first_leaf(space_id: SpaceId, root_node_id: NodeId, space_manager: &mut SpaceManager) -> NodeId` | `pub(crate)` |
| `window_node_find_last_leaf` | `src/view.c:430` | event loop | `SpaceManager` | `pub(crate) fn window_node_find_last_leaf(space_id: SpaceId, root_node_id: NodeId, space_manager: &mut SpaceManager) -> NodeId` | `pub(crate)` |
| `window_node_find_prev_leaf` | `src/view.c:439` | event loop | `SpaceManager` | `pub(crate) fn window_node_find_prev_leaf(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> Option<NodeId>` | `pub(crate)` |
| `window_node_find_next_leaf` | `src/view.c:454` | event loop | `SpaceManager` | `pub(crate) fn window_node_find_next_leaf(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> Option<NodeId>` | `pub(crate)` |
| `window_node_rotate` | `src/view.c:469` | event loop | `SpaceManager` | `pub(crate) fn window_node_rotate(space_id: SpaceId, node_id: NodeId, degrees: i32, space_manager: &mut SpaceManager)` | `pub(crate)` |
| `window_node_mirror` | `src/view.c:494` | event loop | `SpaceManager` | `pub(crate) fn window_node_mirror(space_id: SpaceId, node_id: NodeId, axis: WindowNodeSplit, space_manager: &mut SpaceManager) -> NodeId` | `pub(crate)` |
| `window_node_fence` | `src/view.c:509` | event loop | `SpaceManager` | `pub(crate) fn window_node_fence(space_id: SpaceId, node_id: NodeId, direction: i32, space_manager: &mut SpaceManager) -> Option<NodeId>` | `pub(crate)` |
| `view_find_min_depth_leaf_node` | `src/view.c:525` | event loop | `SpaceManager` | `fn view_find_min_depth_leaf_node(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> Option<NodeId>` | private |
| `area_is_in_direction` | `src/view.c:541` | any | — | `pub(crate) fn area_is_in_direction(first_area: &Area, first_area_max_point: CGPoint, second_area: &Area, second_area_max_point: CGPoint, direction: i32) -> bool` | `pub(crate)` |
| `area_distance_in_direction` | `src/view.c:563` | any | — | `pub(crate) fn area_distance_in_direction(first_area: &Area, first_area_max_point: CGPoint, second_area: &Area, second_area_max_point: CGPoint, direction: i32) -> i32` | `pub(crate)` |
| `view_find_window_node_in_direction` | `src/view.c:583` | event loop | `SpaceManager`, `WindowManager` | `pub(crate) fn view_find_window_node_in_direction(space_manager: &mut SpaceManager, space_id: SpaceId, source_node_id: NodeId, direction: i32, window_manager: &mut WindowManager) -> Option<NodeId>` | `pub(crate)` |
| `view_find_window_node` | `src/view.c:612` | event loop | `SpaceManager` | `pub(crate) fn view_find_window_node(space_manager: &mut SpaceManager, space_id: SpaceId, window_id: WindowId) -> Option<NodeId>` | `pub(crate)` |
| `view_remove_window_node` | `src/view.c:621` | event loop | `SpaceManager`, `DisplayManager`, `WindowManager`, `MouseDragState` | `pub(crate) fn view_remove_window_node(space_manager: &mut SpaceManager, space_id: SpaceId, window_id: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState) -> Option<NodeId>` | `pub(crate)` |
| `view_stack_window_node` | `src/view.c:730` | event loop | `SpaceManager` | `pub(crate) fn view_stack_window_node(space_id: SpaceId, node_id: NodeId, window_id: WindowId, space_manager: &mut SpaceManager)` | `pub(crate)` |
| `view_add_window_node_with_insertion_point` | `src/view.c:751` | event loop | `SpaceManager`, `DisplayManager`, `WindowManager` | `pub(crate) fn view_add_window_node_with_insertion_point(space_manager: &mut SpaceManager, space_id: SpaceId, window_id: WindowId, insertion_point: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> Option<NodeId>` | `pub(crate)` |
| `view_add_window_node` | `src/view.c:814` | event loop | `SpaceManager`, `DisplayManager`, `WindowManager` | `pub(crate) fn view_add_window_node(space_manager: &mut SpaceManager, space_id: SpaceId, window_id: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> Option<NodeId>` | `pub(crate)` |
| `view_find_window_list` | `src/view.c:819` | event loop | `SpaceManager` | `pub(crate) fn view_find_window_list(space_manager: &mut SpaceManager, space_id: SpaceId) -> Vec<WindowId>` | `pub(crate)` |
| `view_is_invalid` | `src/view.c:840` | event loop | `SpaceManager` | `pub(crate) fn view_is_invalid(space_manager: &mut SpaceManager, space_id: SpaceId) -> bool` | `pub(crate)` |
| `view_is_dirty` | `src/view.c:845` | event loop | `SpaceManager` | `pub(crate) fn view_is_dirty(space_manager: &mut SpaceManager, space_id: SpaceId) -> bool` | `pub(crate)` |
| `view_flush` | `src/view.c:850` | event loop | `SpaceManager`, `WindowManager` | `pub(crate) fn view_flush(space_manager: &mut SpaceManager, space_id: SpaceId, window_manager: &mut WindowManager)` | `pub(crate)` |
| `view_serialize` | `src/view.c:860` | event loop | `SpaceManager`, `DisplayManager`, `WindowManager` | `pub(crate) fn view_serialize(response: &mut Response, space_manager: &mut SpaceManager, space_id: SpaceId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `view_update` | `src/view.c:968` | event loop; also main at start-up | `SpaceManager`, `DisplayManager`, `WindowManager` | `pub(crate) fn view_update(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `view_create` | `src/view.c:986` | event loop; also main at start-up | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn view_create(space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> SpaceId` | `pub(crate)` |
| `view_clear` | `src/view.c:1017` | event loop | `SpaceManager`, `DisplayManager`, `WindowManager`, `MouseDragState` | `pub(crate) fn view_clear(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState)` | `pub(crate)` |
| `view_destroy` | `src/view.c:1033` | event loop | `SpaceManager`, `WindowManager`, `MouseDragState` | `pub(crate) fn view_destroy(space_manager: &mut SpaceManager, space_id: SpaceId, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState)` | `pub(crate)` |

52 functions, 52 rows. `src/view.h` declares 27 of them and declares nothing else; the other 25
are definitions in `src/view.c`.

---

## Dead functions — not translated

None. Every function declared in `src/view.h` has a definition in `src/view.c`, every function
defined in `src/view.c` has at least one live caller, and neither file contains a `#if 0` branch.
`DECISIONS.md` 5 removes nothing from this pair.

| C name | why it is not translated |
| --- | --- |
| — | — |

The one `DECISIONS.md` 5 removal that *touches* these files is `TIME_FUNCTION` at `src/view.c:862`
(inside `view_serialize`), which is the `PROFILE` machinery of `misc/timer.h` and expands to
nothing in every build the daemon ships. It is a macro, not a function, so it has no row.

---

## The three `view.h` macros that become functions

Not C functions, so not rows above, but wave 1 writes them into `src/view.rs` and other modules
call them.

| C | Rust | visibility |
| --- | --- | --- |
| `AX_ABS(a, b)` (`view.h:4`) | folded into `ax_diff` — no separate function (`GLOSSARY.md` §7.2) | — |
| `AX_DIFF(a, b)` (`view.h:5`) | `pub(crate) fn ax_diff(first: f64, second: f64) -> bool` | `pub(crate)` — called only from `event_loop.c:709-814` |
| `view_check_flag` / `view_clear_flag` / `view_set_flag` (`view.h:218-220`) | inherent methods on `View` over `flags: u64` tested with `ViewFlag` (`GLOSSARY.md` §5.5, §13); `event_loop.c`, `space_manager.c`, `window_manager.c` and `message.c` all use them | `pub(crate)` |

---

## Rust-only helpers this module owns

Already fixed by `patterns/state-and-ownership.md`; repeated here so wave 1 does not re-derive
them. They have no C original, so §2.2 step 2 is vacuous: handles first, managers appended in the
step 3 order.

| Rust name | source | signature | visibility |
| --- | --- | --- | --- |
| `view_free_node` | §5.4 — the single free funnel | `pub(crate) fn view_free_node(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)` | `pub(crate)` |
| `window_node_collect_subtree_post_order` | §4.4 — phase 1 of recipe R4 | `fn window_node_collect_subtree_post_order(view: &View, node_id: NodeId, out: &mut Vec<NodeId>)` | private |
| `View::node` / `node_mut` / `find_node` / `find_node_mut` | §5.1 — the only four arena accessors | as §5.1 spells them | `pub(crate)` |
| `View::allocate_node` | §5.3 | `pub(crate) fn allocate_node(&mut self) -> NodeId` | `pub(crate)` |

---

## Per-row notes where the mechanical rule needed a reading

Numbered so another module's agent can cite them.

**N1 — `struct view *` plus `struct window_node *` in one C signature.** Step 1 says a
`struct window_node *` becomes a `(SpaceId, NodeId)` pair and step 2 says a `struct view *`
becomes `&mut SpaceManager` + `SpaceId`. Applied literally to
`window_node_get_split(struct view *view, struct window_node *node)` that emits `space_id`
twice. The node's `SpaceId` **is** the view's `SpaceId` — a node is only ever reached through its
own view — so one `space_id` is emitted, at the `struct view *` position, and the node
contributes only its `NodeId`. Affects `window_node_get_split` (`:136`),
`area_make_pair_for_node` (`:186`), `window_node_split` (`:277`), `window_node_update` (`:324`)
and `view_find_window_node_in_direction` (`:583`).

**N2 — `window_node_swap_window_list` really does take two space ids.** Its two nodes can live in
different views: `window_manager.c:2038` calls it under `a_view->sid != b_view->sid`
(`:2016-2036` moves the windows between the two spaces first), and `mouse_handler.c:161` sits
directly above the same cross-space branch (`:163`). `GLOSSARY.md` §1 and
`patterns/state-and-ownership.md` §3.1 require the pair for a node handle that crosses a `View`
boundary, so the signature is `(a_space_id, a_node_id, b_space_id, b_node_id, space_manager)` —
the only row in this file where the C's two node pointers do **not** collapse onto one
`space_id`.

**N3 — `struct window *window` that is only read for `->id` contributes no `WindowManager`.**
§2.1 counts the owning manager of a handle the function *resolves*. `window_node_split`
(`:299-309`), `view_stack_window_node` (`:745`, `:747`), `view_remove_window_node` (`:623`,
`:631`, `:636`, `:646`) and `view_add_window_node_with_insertion_point` (`:755-756`) read
`window->id` and nothing else, which under `DECISIONS.md` 14 *is* the handle — no
`WindowManager::window` lookup happens. This is why `view_stack_window_node` and
`window_node_split` take no `&mut WindowManager`, and it agrees with `state-access.tsv`, which
lists no `g_window_manager` for either. `window_node_capture_windows` (`:362`) is the contrasting
case: it calls `window_manager_find_window`, so it does take one.

**N4 — `area_make_pair` returns its two areas instead of writing through pointers.** The C
out-parameters are `&node->left->area` and `&node->right->area` (`:192`) — two distinct slots of
one `Vec<Option<WindowNode>>` under `DECISIONS.md` 15. Two simultaneous `&mut` into that arena do
not exist, and `parent_area` is a third slot of the same `Vec`, so it is taken by value (`Area`
is `Copy`). `(Area, Area)` is the only shape that compiles at both call sites (`view.c:192`,
`window_manager.c:1894`, where the outputs are the two locals `GLOSSARY.md` §10.4 names
`candidate_first_child_area` / `candidate_second_child_area`). `area_is_in_direction` and
`area_distance_in_direction` keep `&Area` because they only read, and two shared borrows of the
arena coexist happily.

**N5 — `window_node_capture_windows` keeps its out-parameter.** §2.2's "out-parameter counts
collapse into the returned `Vec`" does not apply: the buffer itself is the out-parameter, and
`mouse_handler.c:176-177`, `window_manager.c:1877-1878` and `:2042-2048` push into the **same**
buffer from two consecutive calls. `patterns/state-and-ownership.md` §5, `ts_buf_push` row, fixes
it as `&mut Vec<WindowCapture>`, kept at the C position (second).

**N6 — `view_find_window_list`'s `int *window_count` disappears.** It is derived from the same
buffer, so §2.2 collapses it into the returned `Vec<WindowId>`'s length. The return is a plain
`Vec`, not `Option<Vec>`: `view.c:819-838` cannot return NULL, and the one caller
(`window_manager.c:2543`) only loops over it.

**N7 — `window_node_index_of_window` returns `i32`, not `Option<usize>`.** `DECISIONS.md` 32:
index `0` is an observable sentinel. `view.c:396` returns `0` for "not found" and
`window.c:601` adds 1 and prints it as `stack-index`, so "not found" and "index 0" are
indistinguishable by design and must stay so.

**N8 — `window_node_find_first_leaf` / `_last_leaf` return a bare `NodeId`.** C never returns
NULL for a non-NULL root (`view.c:421-437`), and every call site passes `view->root`, which is
`ROOT_NODE_ID` and `Some` for the life of the `View` (§5.2). `_prev_leaf` / `_next_leaf` return
`Option<NodeId>` because `view.c:441` and `:456` return NULL at the root — that NULL is the
loop terminator at `view.c:594`, `:614`, `:826`, `window_manager.c:1146` and `:1165`.

**N9 — `window_node_fence` takes a bare `NodeId`.** `view.c:511` guards `if (!node) return NULL`,
but its only caller NULL-checks first: `window_manager.c:376-377` returns
`WINDOW_OP_ERROR_INVALID_SRC_NODE` before any of the four calls at `:382-385`. An
`Option<NodeId>` parameter no call site can populate would be noise; the guard becomes
unreachable and is dropped. The **return** stays `Option<NodeId>` — `view.c:522` is a live NULL.

**N10 — `view_create` inserts the `View` and returns its handle.** `struct view *` is owned by
`Table<SpaceId, View>` (§3.2), so there is no pointer to hand back; §2.2's "a returned record
pointer becomes `Option` of its handle" gives `SpaceId`, and it is not optional because
`view.c:986-1015` cannot return NULL. The insertion moves *into* `view_create`: `view.c:1009`
calls `view_update`, which resolves the view through `SpaceManager::view`, so the `View` must
already be in the table by then. The `table_add` at `space_manager.c:108` and `:1226` therefore
disappears from both callers. Nothing between the C's `view_create` return and its caller's
`table_add` reads `g_space_manager.view`, so the reordering is unobservable — one
`DEVIATIONS.md` line.

**N11 — `view_destroy` must run before the view leaves the table.** `event_loop.c:987-989` does
`table_remove`, then `view_destroy(view)`, then `free(view)`. Under §4.4 `view_destroy` resolves
the view through `SpaceManager::view`, so `EVENT_HANDLER(SLS_SPACE_DESTROYED)` must call
`view_destroy(space_manager, space_id, window_manager, mouse_drag_state)` **first** and only then
`space_manager.view.remove(&space_id)` (which yields the `View` and drops it — `View` has no
`Drop` impl, exactly as `view_destroy` does not `free(view)`). `view_destroy` reaches only
`window_manager_remove_managed_window`, `insert_feedback_destroy` and `update_window_notifications`,
none of which read `g_space_manager.view`, so the swap is unobservable. This is a constraint on
the `event_loop` module, recorded here because the signature is what forces it — one
`DEVIATIONS.md` line.

**N12 — `MouseDragState` appears on four rows that the C never gave it.** `view_free_node` (§5.4)
clears `MouseDragState::feedback_node` when the node's slot is freed, which the C never does
(`sweeps/globals-and-ownership.md` §9.1). Every function that frees a node therefore takes
`&mut MouseDragState`: `window_node_destroy` (`:335`), `view_remove_window_node` (`:621`),
`view_clear` (`:1017`) and `view_destroy` (`:1033`). `view_add_window_node_with_insertion_point`
and `window_node_update` allocate but never free, so they do not take it.

**N13 — `DisplayManager` appears only where `view_update` is reached.** Per §5.5's closing
bullet: `view_update` → `display_bounds_constrained` (`display.c:123`) reads
`DisplayManager::mode`, `::top_padding` and `::bottom_padding`. That is the whole reason
`view_remove_window_node`, `view_add_window_node*`, `view_clear` and `view_create` take one.
`window_node_destroy` and `view_destroy` do not call `view_update` and do not take it.
`view_serialize` takes one for a different reason: `display_manager_display_id_arrangement`
(`display_manager.c:165`, `view.c:907`).

**N14 — `view_serialize`'s `WindowManager`.** `space_window_list` (`view.c:915`) reaches
`space_window_list_for_connection` (`space.c:17`), which reads `WindowManager::window` at
`space.c:45` and `:58`. Same reason `view_find_window_node_in_direction` (`view.c:586`) takes
one. `state-access.tsv` lists `g_window_manager` in both rows, and §2.1 uses exactly this pair as
its worked case for "every manager parameter is `&mut`, no read-only variants".

**N15 — handle parameter names.** `node` → `node_id`, `a_node` / `b_node` → `a_node_id` /
`b_node_id`, `parent` → `parent_id`, `child` → `child_id` (as §5.5 writes them); `window` →
`window_id`; `sid` and `struct view *view` → `space_id` (`GLOSSARY.md` §10.2). Two C names do not
say what kind of thing they hold once they are handles, so they are spelled out rather than left
as `root_id` / `source_id`: `root` → `root_node_id` (`view.c:421`, `:430`) and `source` →
`source_node_id` (`view.c:583`). `dir` → `direction` and `insert_dir` → `insert_direction`
(`GLOSSARY.md` §10.3); `r1` / `r2` / `r1_max` / `r2_max` → `first_area` / `second_area` /
`first_area_max_point` / `second_area_max_point` (`GLOSSARY.md` §10.4); `rsp` → `response`
(same). `balance_node_add`'s bare `a` / `b` are not in `GLOSSARY.md` §10.3, so they follow the
`string_equals` / `psn_equals` rows of the same table: `first` / `second`.

**N16 — `view_add_window_node_with_insertion_point`'s `uint32_t insertion_point` is a
`WindowId`.** `GLOSSARY.md` §13 settles it: `View::insertion_point` is a window id, not a
`WindowInsertionPoint`, because `view.c:646-647`, `:765` and `window_manager.c:1793` assign a
window id to it. `SpaceManager::window_insertion_point` is the enum, and it is read at
`view.c:786-790` through the manager, never through this parameter.

---

## Where this file departs from an illustrative sketch elsewhere

`patterns/state-and-ownership.md` §2.2 is the binding rule and says so ("the two must agree
literally, parameter for parameter"). Four inline call sketches in §4 and §5 of the same document
write the same functions with the manager block as a **prefix**. Those sketches are illustrating
a borrow recipe, not fixing a signature; the rows above are §2.2's output and are what wave 1
pastes.

| sketch | where | this file |
| --- | --- | --- |
| `window_node_flush(space_manager, window_manager, space_id, ROOT_NODE_ID)` | §4.3, line 750 | `window_node_flush(space_id, node_id, window_manager, space_manager)` |
| `view_find_window_node(view, window_id)` taking `&View` | §5.5 | `view_find_window_node(space_manager, space_id, window_id)` — §2.2 step 2 turns every `struct view *` into `&mut SpaceManager` + `SpaceId`, never `&View` |
| `window_node_update(space_manager, window_manager, space_id, parent_id)` | §5.5 | `window_node_update(space_manager, space_id, node_id, window_manager)` — `window_manager` is appended by step 3, after the whole C parameter list |
| `window_node_balance(space_manager, space_id, ROOT_NODE_ID, auto_balance)`, `window_node_clear_zoom(view, parent_id)`, `view_update(display_manager, space_manager, window_manager, space_id)` | §5.5 | `window_node_balance(space_id, node_id, axis_flag, space_manager)`, `window_node_clear_zoom(space_id, node_id, space_manager)`, `view_update(space_manager, space_id, display_manager, window_manager)` |

The three signatures §4 and §5 state as *declarations* rather than call sketches —
`insert_feedback_show` (§2.3 example 1), `window_node_destroy` (§4.4) and
`view_remove_window_node` (§5.5) — already follow §2.2 exactly and are reproduced above
character for character.

`patterns/state-and-ownership.md` §2.2 also writes `FILE *rsp` as
`response: &mut dyn std::io::Write`. `DECISIONS.md` 28, `patterns/message-and-serialisation.md`
§7 and `GLOSSARY.md` §10.4 all spell it `response: &mut Response`, and `Response` owns the
failure prefix and the silent case that a bare `Write` cannot express. `view_serialize` takes
`&mut Response`.

---

## Functions in other modules that `crate::view` calls

The argument shapes `view.rs` will pass. Each is that module's own row to fix; a disagreement is
a fixed-point error, not something to patch here.

| callee | module | call sites in `view.c` | shape `crate::view` passes |
| --- | --- | --- | --- |
| `update_window_notifications` | `crate::event_loop` | `:45`, `:111` | `(window_manager: &mut `WindowManager`, space_manager: &`SpaceManager`)` — `static` in the C (`event_loop.c:16`) and reachable only through the unity build; it must become a real `pub(crate)` item |
| `window_manager_remove_managed_window` | `crate::window_manager` | `:341`, `:1024` | `(window_manager: &mut `WindowManager`, window_id: WindowId)` |
| `window_manager_find_window` | `crate::window_manager` | `:362` | `(window_manager: &mut `WindowManager`, window_id: WindowId) -> Option<WindowId>` — used only as a liveness test before pushing a `WindowCapture` |
| `window_manager_animate_window_list` | `crate::window_manager` | `:378` | `(window_list: &[WindowCapture], window_manager: &mut `WindowManager`)` — fixed verbatim by §2.3 example 3; the `int window_count` argument is gone |
| `window_manager_find_rank_of_window_in_list` | `crate::window_manager` | `:600` | `(window_id: WindowId, window_list: &[WindowId]) -> i32` — the `(pointer, count)` pair collapses to one slice; returns `i32::MAX` when absent |
| `space_window_list` | `crate::space` | `:586`, `:915` | `(space_id: SpaceId, include_minimized: bool, window_manager: &mut `WindowManager`) -> Option<Vec<WindowId>>` — the `int *count` out-parameter collapses; the NULL return is load-bearing at `:587` |
| `space_display_id` | `crate::space` | `:907`, `:970` | `(space_id: SpaceId) -> DisplayId` — no manager; `g_connection` only |
| `space_is_visible` | `crate::space` | `:852`, `:955` | `(space_id: SpaceId) -> bool` |
| `space_is_user` | `crate::space` | `:1000` | `(space_id: SpaceId) -> bool` |
| `space_is_fullscreen` | `crate::space` | `:962` | `(space_id: SpaceId) -> bool` |
| `space_manager_mission_control_index` | `crate::space_manager` | `:885` | `(space_id: SpaceId) -> i32` — no manager; `g_connection` only |
| `space_manager_get_label_for_space` | `crate::space_manager` | `:892` | `(space_manager: &mut `SpaceManager`, space_id: SpaceId) -> Option<&SpaceLabel>` — `view.c:893` reads `space_label->label`, a `String` (§3.2) |
| `display_manager_display_id_arrangement` | `crate::display_manager` | `:907` | `(display_id: DisplayId, display_manager: &mut `DisplayManager`) -> i32` |
| `display_bounds_constrained` | `crate::display` | `:971` | `(display_id: DisplayId, ignore_external_bar: bool, display_manager: &mut `DisplayManager`) -> CGRect` |
| `window_level`, `window_sub_level` | `crate::window` | `:24`, `:25` | `(window_id: WindowId) -> i32` each — no manager; `g_connection` only |
| `sls_window_disable_shadow` | `crate::misc::helpers` | `:21` | `(window_id: WindowId)` |
| `cgrect_clamp_x_radius`, `cgrect_clamp_y_radius` | `crate::misc::helpers` | `:89` | `(rect: CGRect, radius: f32) -> f32` each |
| `json_bool` | `crate::misc::helpers` | `:948`, `:955`, `:962` | `(value: bool) -> &'static str` |
| `cfstring_copy` | `crate::misc::helpers` | `:877` (`ts_cfstring_copy`) | `(string: &CFString) -> Option<String>` — the arena copy becomes an owned `String` (`DECISIONS.md` 17); `View::uuid` is `Option<CFStringOwned>`, so the `NULL`-uuid crash of `view.c:877` becomes a `None` arm printing `"<unknown>"` |
| `workspace_is_macos_sequoia`, `workspace_is_macos_tahoe` | `crate::workspace` | `:44`, `:110` | `() -> bool` each — statics only |
| `in_range_ii` | `crate::misc::macros` | `:153` | `macro_rules!`, inclusive on both ends |
| `Table::add`, `Table::remove` | `crate::misc::table` | `:43`, `:108` | `window_manager.insert_feedback.add(window_id, (space_id, node_id))` and `.remove(&window_id)` — the table is `Table<WindowId, (SpaceId, NodeId)>` (§3.2), keyed on `node.window_order[0]`, never on the node |
