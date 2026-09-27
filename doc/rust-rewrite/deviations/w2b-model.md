# Deviations — `w2b-model` (second halves of `src/view.rs`, `src/window.rs`, `src/space_manager.rs`)

## `src/view.c:525-1042` → `src/view.rs`

`src/view.c:525`, `:583`, `:612`, `:621`, `:730`, `:751`, `:819`, `:840`, `:845`, `:850`, `:860`, `:968`, `:1017`, `:1033` | every `struct view *` and `struct window_node *` parameter was a pointer the caller held live for the duration of the call | the `SpaceId` / `(SpaceId, NodeId)` handle is re-resolved through `SpaceManager::view` at each use; a table miss, which cannot happen in C, returns early for a `()` function and otherwise returns `None` or `false`, and `view_serialize` writes nothing for a missing view

`src/view.c:527-538` | `view_find_min_depth_leaf_node` ran its breadth-first search in `struct window_node *list[256]`, writing `list[256]` at `i == 127` and further past the array afterwards (`DECISIONS.md` 4) | a `VecDeque<NodeId>` seeded with the start node, popped from the front, `left` then `right` pushed behind; the 256-iteration cap goes with the array, so a tree whose first 128 breadth-first nodes are all internal returns its shallowest leaf instead of overrunning (`patterns/state-and-ownership.md` §5.6)

`src/view.c:586-587` | `space_window_list` returned `NULL` when `SLSCopyWindowsWithOptionsAndTags` failed or reported no window, and a non-`NULL` zero-length list when it reported windows none of which passed the filter; only the `NULL` case returned early, the other searched with every rank at `INT_MAX` | `space_window_list` returns `Vec<WindowId>` (`w1-space`), so the early return is `window_list.is_empty()` and the "reported but all filtered out" case now returns `None` instead of the nearest leaf by distance

`src/view.c:741-748` | `view_stack_window_node` on a node already holding `NODE_MAX_WINDOW_COUNT` windows wrote one element past `window_list` and `window_order` and raised `window_count` to 33 (`DECISIONS.md` 4) | returns without stacking when `window_count >= NODE_MAX_WINDOW_COUNT`; the bound is at 32, not the `window_count + 1 >= NODE_MAX_WINDOW_COUNT` that `patterns/state-and-ownership.md` §5.6 sketches, because a node holding 31 windows is stacked to 32 without any overrun in C and that defined behaviour is kept

`src/view.c:793-796` | when neither the insertion point, nor the `window_insertion_point` policy, nor `view_find_min_depth_leaf_node` produced a leaf, `window_node_split(view, NULL, window)` dereferenced `NULL` (`DECISIONS.md` 4) | `view_add_window_node_with_insertion_point` returns `None` before the split

## `src/window.c:713-1144` → `src/window.rs`

`src/window.c:719`, `:726`, `:1006`, `:1027` | `window_property_title_ts`, `window_title_ts`, `window_role_ts` and `window_subrole_ts` handed back the `NULL` that `ts_cfstring_copy` returns when `CFStringGetCString` fails, which `ts_string_escape` (`src/window.c:186`, `:486`) and `regexec` (`src/rule.c:117-119`) then dereferenced | each returns `String` and turns that `NULL` into the empty string, the value C already returns for a missing title, role or subrole

`src/window.c:775-778`, `:789-792`, `:803-806` | `Boolean result` was left uninitialised and read if `AXUIElementIsAttributeSettable` succeeded without writing it (`DECISIONS.md` 4) | `result` starts at `0`; the `result = 0` on failure is kept

`src/window.c:944` | `SLSGetWindowSubLevel__Internal` called `CGSGetConnectionPortById` without testing it, relying on its only caller `window_sub_level` (`src/window.c:956`) to have done so | the runtime-resolved pointer is an `Option`, and a `None` returns sub-level `0` before building the message (`patterns/ffi-objc-and-os.md` §15.7); unreachable through `window_sub_level`

`src/window.c:1037` | `CFEqual(value, window->application->ref)` dereferenced `window->application`, which `src/event_loop.c:281` sets to `NULL` | an application that no longer resolves through `WindowManager::application`, or whose element is `NULL`, makes the comparison false, so a window reporting a parent element is not root; its only caller `window_create` runs while the application is still in the table

## `src/space_manager.c:633-1233` → `src/space_manager.rs`

`src/space_manager.c:641`, `:645` | `space_manager_last_space` indexed `display_spaces_count-1` and `spaces_count-1` with no bounds check, reading index `-1` of an empty array | returns `SpaceId(0)` when either array is empty, as `space_manager_first_space` already does for `:622`, `:625` (`DECISIONS.md` 4)

`src/space_manager.c:642`, `:646` | `CFDictionaryGetValue(display_ref, CFSTR("Spaces"))` and `CFDictionaryGetValue(space_ref, CFSTR("id64"))` were used with no `NULL` check | `.unwrap()`, the same panic-then-abort the four sibling walkers already use (`DECISIONS.md` 4, 7)

`src/space_manager.c:668-669`, `:689-690` | `objc_getClass("SLSBridgedMoveWindowsToManagedSpaceOperation")` was not checked, so a missing class sent `alloc` and `initWithWindows:spaceID:` to nil and handed nil to `SLSPerformAsynchronousBridgedWindowManagementOperation` | the class lookup is an `Option` and a miss returns before the message sends, releasing the window list (`patterns/ffi-objc-and-os.md` §24)

`src/space_manager.c:681`, `:702` | `SLSSetWindowListWorkspace` was handed the caller's `uint32_t *` window list directly | the `&[WindowId]` is copied into a `Vec<u32>` first, because `WindowId` carries no `#[repr(transparent)]` guarantee; the same ids are passed in the same order

`src/space_manager.c:760-771` | `table_find` could return `NULL` for either view and both were dereferenced unconditionally after `table_remove` (`DECISIONS.md` 4) | `Table::remove` yields `Option<View>`; a missing `a_view` returns `SPACE_OP_ERROR_INVALID_SRC`, a missing `b_view` puts `a_view` back and returns `SPACE_OP_ERROR_INVALID_DST`, and both paths restore `window_animation_duration` before returning (`patterns/state-and-ownership.md` §4.5)

`src/space_manager.c:752`, `:796` | `__asm__ __volatile__ ("" ::: "memory")` compiler barriers around the `window_animation_duration` save and restore | `std::sync::atomic::compiler_fence(Ordering::SeqCst)` at the same two points, and on the two early returns above

`src/space_manager.c:1126`, `:1217` | `if (!display_list) return;` after `display_manager_active_display_list`, which never returns `NULL` | the dead guard is not translated; zero active displays run the loop zero times, exactly as the C does after the guard (`DECISIONS.md` 5)

`src/space_manager.c:1138-1139` | `debug` printed `application->name` through a `struct application *` held in `applications_to_refresh` | the `ProcessId` is resolved through `WindowManager::application`, and a miss prints `(null)` as a `NULL` `%s` does (`DECISIONS.md` 29)

`src/space_manager.c:1156-1164` | two VLAs sized by `sm->view.count` held every `struct view *` and its borrowed `CFStringRef uuid`, while the loop removed and re-added views of that same table | a `Vec<Option<SpaceId>>` of the keys in bucket order; each entry is re-resolved for its `uuid` before `CFEqual` and nulled when matched, so the same views are compared in the same order (`patterns/state-and-ownership.md` §4.5)

`src/space_manager.c:1213`, `:1225-1226` | `table_init` then `view_create` + `table_add` per space | `space_manager.view = Table::new(23, hash_view_key)`, and `view_create` performs the insertion itself (`state-access/view.md` N10), so the loop has no `table_add`
