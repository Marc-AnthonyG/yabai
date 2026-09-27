# Deviations — `w2-view-part1` (W2-view part 1, `src/view.h` + `src/view.c:1-520` → `src/view.rs`)

`src/view.c:8`, `:105`, `:131`, `:136`, `:151`, `:156`, `:186`, `:198`, `:203`, `:208`, `:213`, `:218`, `:223`, `:242`, `:277`, `:324`, `:335`, `:348`, `:358`, `:374`, `:381`, `:390`, `:399`, `:421`, `:430`, `:439`, `:454`, `:469`, `:494`, `:509` | every `struct view *` and `struct window_node *` parameter was a pointer the caller held live for the duration of the call | the pair `(SpaceId, NodeId)` re-resolved through `SpaceManager::view` at each use (`DECISIONS.md` 14, `state-access/view.md`); a table miss — which cannot happen in C, where the caller already held the pointer — returns early for a `()` function and otherwise returns the zero value of the return type (`false`, `0`, `None`, the node id itself), except `window_node_get_child` and `window_node_get_ratio`, which fall through to the space default exactly as the C does when the node carries no override

`src/view.c:15` | `if (!node->feedback_window.id)` re-tested the SLS window id, so a `SLSNewWindowWithOpaqueShapeAndContext` that left the id `0` was retried on the next call | `if node.feedback_window.is_none()` tests the `Option<FeedbackWindow>` of `GLOSSARY.md` §3.10, which is `Some` after the first attempt, so a failed creation is not retried

`src/view.c:49-84` | `clip_x`, `clip_y`, `clip_w` and `clip_h` were uninitialised locals and `switch (node->insert_dir)` had no `default:`, so an `insert_dir` of `0` read indeterminate values (`DECISIONS.md` 4) | the `_` arm is `unreachable!()`; every call site sets `insert_dir` first (`view.c:327`, `:686`, `window_manager.c:1794`, `event_loop.c:1329`), as `files/view-and-tests.md` §2.4 requires

`src/view.c:98` | `CGContextResetClip(node->feedback_window.context)` was called unconditionally on a context that is NULL whenever `SLWindowContextCreate` failed | the `objc2-core-graphics` binding takes `&CGContext`, so the call sits under `if let Some(..)` and a NULL context skips it instead of dereferencing NULL; the other eight `CGContext*` calls of the function take `Option<&CGContext>` and are handed `None`, which is the C's NULL argument

`src/view.c:114-116` | `insert_feedback_destroy` ordered the SLS window out, called `CGContextRelease` and called `SLSReleaseWindow` inline | those three calls are `FeedbackWindow::drop`, in the same order (`patterns/state-and-ownership.md` §5.4), so freeing a node's slot also releases its overlay; `CGContextRelease` has no binding in `objc2-core-graphics`, so the context is released by handing it to `CFRetained` and dropping it, which is the same `CFRelease`

`src/view.c:117` | `memset(&node->feedback_window, 0, sizeof(struct feedback_window))` zeroed the embedded struct | `node.feedback_window.take()`, whose drop is the release sequence above (`patterns/state-and-ownership.md` §5.4)

`src/view.c:105`, `:108` | `insert_feedback_destroy` dereferenced the `struct window_node *` it was handed, which at `event_loop.c:1192`, `:1298`, `:1335` and `window_manager.c:1762` comes from `g_mouse_state.feedback_node` or `g_window_manager.insert_feedback` and can name a node that was already freed | the node is resolved with `View::find_node_mut` (`patterns/state-and-ownership.md` §5.1), so a stale handle is a miss and the function returns

`src/view.c:192` | `area_make_pair(split, gap, ratio, &node->area, &node->left->area, &node->right->area)` dereferenced `node->left` and `node->right` with no NULL check | `area_make_pair` returns the two areas by value (`state-access/view.md` N4, because the three areas are three slots of one `Vec<Option<WindowNode>>`) and each write sits under `if let Some(..)`

`src/view.c:330-331` | `window_node_update` recursed into `node->left` and `node->right` with no NULL check, which a node with exactly one child would have dereferenced | both calls sit under `if let Some(..)`

`src/view.c:225-226`, `:251-252`, `:337-338`, `:353-354`, `:369-370`, `:423`, `:432`, `:444`, `:446`, `:451`, `:459`, `:461`, `:466`, `:489-490`, `:497-498` | every descent or recursion through `node->left` / `node->right` / `node->parent->left` / `node->parent->right` dereferenced the child pointer without a NULL check | each sits under `if let Some(..)` or a `match`, returning the C's NULL result (`None`, or the current node for the two leaf walks) when the edge is absent

`src/view.c:335-346` | `window_node_destroy` walked the subtree while calling `window_manager_remove_managed_window` and `free` from inside the walk | recipe R4 of `patterns/state-and-ownership.md` §4.4: `window_node_collect_subtree_post_order` builds the post-order `Vec<NodeId>` under `&View`, then the loop replays it against the three managers; the C's `free(node)` is `view_free_node`

`src/view.c:346` | `free(node)` returned the node to the allocator and left `g_mouse_state.feedback_node` pointing into freed memory (`sweeps/globals-and-ownership.md` §9.1) | `view_free_node` clears `MouseDragState::feedback_node` whenever it names the slot being freed, which the C never does anywhere

`src/view.c:365` | `ts_buf_push` appended the capture to a stretchy buffer in the 8 MB temp-storage arena, valid only until the `ts_reset()` at the bottom of the event handler | the out-parameter is `&mut Vec<WindowCapture>` owned by the caller (`DECISIONS.md` 17); `window_node_flush`'s buffer is dropped when `window_node_flush` returns rather than at the end of the event

`src/view.c:378` | `window_manager_animate_window_list(window_list, ts_buf_len(window_list))` passed the buffer and its length separately, and the `if (window_list)` guard existed only because `ts_buf_len(NULL)` is `0` | the pair collapses to one `&[WindowCapture]` (`state-access/view.md`) and the guard becomes `!window_list.is_empty()`, which is the same test

`src/view.c:403` | `uint32_t tmp_window_count` carried an `int window_count` through the swap, a silent signed/unsigned round trip | `window_count` stays `i32` on both sides of the swap; the round trip was value-preserving for the whole `0..=32` range, so the divergence is unobservable

`src/view.c:511` | `window_node_fence` opened with `if (!node) return NULL` | the parameter is a bare `NodeId` and the guard is gone; its only caller (`window_manager.c:376-385`) returns `WINDOW_OP_ERROR_INVALID_SRC_NODE` before any of the four calls, so the guard is unreachable (`state-access/view.md` N9)

`src/view.c:170-172`, `:180-182` | `(int)left_width` and `(int)(left_width + 0.5f)` were undefined when the product did not fit in an `int` | Rust's `as i32` truncates toward zero for every in-range value and saturates out of range (`patterns/idioms-and-conventions.md` §7.2); no reachable area produces an out-of-range product, so nothing observable changes

`src/view.c:279-283` | `window_node_split` obtained its two children with `malloc` + `memset(0)` | `View::allocate_node` twice, left then right, over the index arena of `DECISIONS.md` 15 and `patterns/state-and-ownership.md` §5.3; a node id may be reused from the free list, which no comparison in the tree and nothing printed depends on

`src/view.c:291` | the fourth arm of the zoom ternary was `view->root` | `ROOT_NODE_ID`, since `struct view::root` is gone and the root is always `NodeId(0)` (`patterns/state-and-ownership.md` §5.2)
