# Deviations — `w1-view` (W1-view, `src/view.h` + `src/view.c` → `src/view.rs`)

`src/view.h:7-19` | `SPACE_PROPERTY_LIST` was expanded three times by redefining `SPACE_PROPERTY_ENTRY` around each use | one `macro_rules! space_property_list` handing the whole entry list to one callback macro that emits the twelve `u64` constants, `SPACE_PROPERTY_VAL` and `SPACE_PROPERTY_STR` together — the shape `src/misc/helpers.rs:12-68` already uses for `ANIMATION_EASING_TYPE_LIST` (`DECISIONS.md` 42); the per-entry `$entry!(..)` sketch of `patterns/idioms-and-conventions.md` §4.1 does not compile, because a macro in array-element position must expand to a single expression and one in item position must be brace-delimited or semicolon-terminated

`src/view.h:21-26` | `enum space_property` | twelve `pub(crate) const … : u64`, not an enum (`GLOSSARY.md` §2.2, §6.2, `patterns/idioms-and-conventions.md` §6.6)

`src/view.h:57-66`, raced at `src/window_manager.c:560-563` against `:635-642` | `float tx, ty, tw, th` were written every frame by the CVDisplayLink thread without the lock and read by the event-loop thread under it — a data race | `target_x` / `target_y` / `target_width` / `target_height` are `AtomicU32` holding `f32::to_bits`, relaxed both ways, with identical values (`DECISIONS.md` 4, 24, `THREADS.md` §8.1)

`src/view.h:75` | `volatile bool skip` | `AtomicBool` (`DECISIONS.md` 24, `THREADS.md` §8.1)

`src/view.h:68-76` | `struct window_animation` carried both `struct window *window` and `uint32_t wid`, always naming the same window | one field, `window_id: WindowId` (`DECISIONS.md` 14, `GLOSSARY.md` §3.14, §13)

`src/view.h:78-86` | `struct window_animation_context` had no link back to `g_window_manager.window_animations_table`; the display-link callback reached the table through the global | `AnimationContext` carries its own clone of the `Arc<Mutex<Table<WindowId, (Arc<AnimationContext>, usize)>>>` and the callback takes no manager (`DECISIONS.md` 24, ruling 46)

`src/view.h:57-66`, `:78-86` | `THREADS.md` §8.1 additionally makes `WindowProxy::id`, `::frame`, `::level`, `::sub_level` atomics, splits `context` and `image` into a `Mutex<Option<WindowProxyCoreGraphicsObjects>>`, and names a `WindowAnimationHandle` type | `GLOSSARY.md` §3.13-3.15 and §3.1's `window_animations_table` row are what `src/view.rs` writes — `id: WindowId`, `context: *mut CGContext`, `frame: CGRect`, `level: i32`, `sub_level: i32`, `image: Option<*mut CGImage>`, and the table value `(Arc<AnimationContext>, usize)`; ruling 41 makes the glossary win over `THREADS.md` on every type name

`src/view.h:145-149`, `src/view.c:114-116` | `insert_feedback_destroy` was the only site that released the overlay, and only when `node->feedback_window.id` was set, so a node freed with an overlay still attached leaked its SLS window and its `CGContextRef` | `FeedbackWindow` releases both in `Drop`, in the C order — `SLSOrderWindow(connection, id, 0, 0)`, `CGContextRelease(context)`, `SLSReleaseWindow(connection, id)` — so freeing a slot always releases its overlay (`DECISIONS.md` 4, `patterns/state-and-ownership.md` §5.4)

`src/view.h:201-216`, `:154-157` | `struct view::root` was a `struct window_node *` and the tree was a `malloc`ed pointer graph reached through `parent` / `left` / `right` / `zoom` | the tree is an index arena owned by its `View` — `nodes: Vec<Option<WindowNode>>` plus `free_node_ids: Vec<NodeId>`, `Option<NodeId>` for the four edges, the root pinned at `ROOT_NODE_ID` — so the `root` field disappears (`DECISIONS.md` 14, 15, `patterns/state-and-ownership.md` §5.1-5.2)

`src/view.h:218-220` | `view_check_flag(v, x)` was a macro yielding the masked `uint64_t`, not a `bool` | `view_check_flag` and `View::check_flag` return `bool`; all 25 uses sit in `if` or `?:` position, so no call site changes meaning (`patterns/idioms-and-conventions.md` §6.2)

`src/view.c:161-184`, written through at `:192` | `area_make_pair` wrote its results through `struct area *left_area` and `*right_area`, which are two distinct slots of the same node arena, while reading a third slot through `struct area *parent_area` | returns `(Area, Area)` and takes `parent_area` by value; `Area` is `Copy` (`DECISIONS.md` 15, `state-access/view.md` N4)

`src/view.c:511` | `window_node_fence` began `if (!node) return NULL` | the parameter is a bare `NodeId`; `src/window_manager.c:376-377` returns `WINDOW_OP_ERROR_INVALID_SRC_NODE` before all four calls at `:382-385`, so the guard is unreachable and is dropped (`state-access/view.md` N9)

`src/view.c:335`, `:621`, `:1017`, `:1033` | freeing a node never cleared `g_mouse_state.feedback_node`, which kept pointing at freed memory | every free goes through `view_free_node`, which clears `MouseDragState::feedback_node` when the slot it names is freed; `window_node_destroy`, `view_remove_window_node`, `view_clear` and `view_destroy` therefore take `&mut MouseDragState`, which the C never gave them (`DECISIONS.md` 4, `patterns/state-and-ownership.md` §5.4, `sweeps/globals-and-ownership.md` §9.1)

`src/view.c:682` | when `child`'s insert overlay migrated to `parent`, the `g_window_manager.insert_feedback` entry was left keyed on the old `node.window_order[0]` and pointing at the `child` about to be freed | the entry is re-pointed at `parent` as part of the migration (`patterns/state-and-ownership.md` §5.4, `files/view-and-tests.md` §2.6-4)

`src/view.c:819-838` | `view_find_window_list` returned a `ts` buffer plus an `int *window_count` out-parameter | returns `Vec<WindowId>`; the count is the vector's length (`DECISIONS.md` 17, `state-access/view.md` N6)

`src/view.c:862` | `TIME_FUNCTION` opened `view_serialize` | not translated; it is the `PROFILE` machinery of `src/misc/timer.h`, never compiled in any build the daemon ships (`DECISIONS.md` 5)

`src/view.c:877` | `view_serialize` passed `view->uuid` to `ts_cfstring_copy` unchecked, so a `View` whose `uuid` is NULL crashed | `View::uuid` is `Option<CFStringOwned>` and the `None` arm prints `"<unknown>"` (`DECISIONS.md` 4, 32)

`src/view.c:986-1015`, `src/space_manager.c:108`, `:1226` | `view_create` `malloc`ed a `struct view` and returned it; both callers then `table_add`ed it into `g_space_manager.view` | `view_create` inserts the `View` into `SpaceManager::view` itself — `view.c:1009` calls `view_update`, which resolves the view through the manager — and returns the `SpaceId`; both callers lose their `table_add` (`state-access/view.md` N10)

`src/view.c:1018`, `:1034` | `if (view->root)` guarded the body of `view_clear` and of `view_destroy` | always true, because `nodes[0]` is `Some` for the life of the `View`; both guards disappear (`patterns/state-and-ownership.md` §5.2)

`src/event_loop.c:987-989` | `EVENT_HANDLER(SLS_SPACE_DESTROYED)` did `table_remove`, then `view_destroy(view)`, then `free(view)` | `view_destroy` resolves the view through `SpaceManager::view`, so it runs first and `space_manager.view.remove(&space_id)` follows; nothing `view_destroy` reaches reads `g_space_manager.view`, so the swap is unobservable (`state-access/view.md` N11)
