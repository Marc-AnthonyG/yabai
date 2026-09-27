# State access — `space_manager` and `space`

Final Rust signature of every function defined in `src/space_manager.h`, `src/space_manager.c`,
`src/space.h` and `src/space.c`. Wave 1 pastes the **Rust signature** column verbatim.

`src/space_manager.h` and `src/space.h` define no function bodies — they declare `struct
space_label`, `struct space_manager`, `enum space_op_error` and the prototypes only — so every
row below comes from `src/space_manager.c` or `src/space.c`, in definition order within each
file.

## How each row was derived

`patterns/state-and-ownership.md` §2 is the rule, applied in its three steps and no other way:

1. the C parameter list in C order, each pointer rewritten to its handle (`struct window *` →
   `WindowId`, `struct view *` → `&mut SpaceManager` immediately followed by `SpaceId`,
   `struct window_node *` → `(SpaceId, NodeId)`);
2. a manager the C declares stays where C declared it;
3. every remaining manager of `Managers(f)` appended after the whole C parameter list, in the
   `src/yabai.c:27-35` order — `process_manager` (`:28`), `display_manager` (`:29`),
   `window_manager` (`:30`), `space_manager` (`:31`), `mouse_drag_state` (`:33`).

Every manager parameter is `&mut`. Statics are never parameters — `CONNECTION` (`g_connection`)
is read where it is used, which is why `g_connection` appears in almost every `state-access.tsv`
row for these two files and in none of these signatures. `FILE *rsp` becomes
`response: &mut Response` (`DECISIONS.md` 28, `GLOSSARY.md` §10.1/§10.4) and keeps its first
position. A `(pointer, count)` argument pair derived from one buffer collapses into one slice; an
`int *count` out-parameter paired with the returned buffer collapses into the returned `Vec`'s
length.

Return types: a nullable record pointer becomes `Option` of its handle; a non-nullable one becomes
the bare handle (`patterns/idioms-and-conventions.md` §8.4 names `space_manager_find_view` and
`view_create` as the two non-nullable lookups); a returned list becomes `Vec`; `bool` stays
`bool`; observable sentinels stay integers — `SpaceId(0)`, `DisplayId(0)` and the `int` 0 of
`space_manager_mission_control_index` (`DECISIONS.md` 32, `GLOSSARY.md` §1).

Visibility is `pub(crate)` unless `state-access.tsv`'s `called_from_other_files` column is `-`,
in which case the function is a private `fn`. Six functions are private on that test:
`hash_view`, `space_manager_query_view`, `space_manager_move_window_list_to_space`,
`space_manager_find_first_user_space_for_display`, `space_manager_is_space_last_user_space` and
`space_manager_swap_space_with_space_on_display`. Only the last three are `static` in the C; the
other three are declared in `src/space_manager.h` but reached from nowhere outside
`src/space_manager.c`.

Thread context, from `THREADS.md` §1.4: *"Transitively, EVENTLOOP owns all of `window_manager.c`,
`space_manager.c`, `view.c`, `window.c`, `display_manager.c` …"*. Everything here runs on the
event loop. `space_manager_begin` is the one exception — its single call site is `src/yabai.c:337`,
which `patterns/state-and-ownership.md` §1.4 step 3 runs **on the main thread against
`&mut state`**, so it keeps its manager parameters. Three functions run in both contexts, because
`space_manager_begin` reaches them: `space_manager_active_space` (`space_manager.c:1230`),
`space_is_user` (`view.c:1000`, through `view_create`) and `space_display_id` (`view.c:970`,
through `view_create` → `view_update`).

---

## `src/space_manager.c`

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `hash_view` | `src/space_manager.c:4` | any | — | `fn hash_view_key(key: &SpaceId) -> u64` | private |
| `compare_view` | `src/space_manager.c:9` | any | — | *not translated — see the second table* | — |
| `space_manager_query_space` | `src/space_manager.c:14` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_query_space(response: &mut Response, space_id: SpaceId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> bool` | `pub(crate)` |
| `space_manager_query_spaces_for_window` | `src/space_manager.c:26` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_query_spaces_for_window(response: &mut Response, window_id: WindowId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> bool` | `pub(crate)` |
| `space_manager_query_spaces_for_display` | `src/space_manager.c:47` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_query_spaces_for_display(response: &mut Response, display_id: DisplayId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> bool` | `pub(crate)` |
| `space_manager_query_spaces_for_displays` | `src/space_manager.c:68` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_query_spaces_for_displays(response: &mut Response, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> bool` | `pub(crate)` |
| `space_manager_query_view` | `src/space_manager.c:97` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `fn space_manager_query_view(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> Option<SpaceId>` | private |
| `space_manager_find_view` | `src/space_manager.c:103` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_find_view(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> SpaceId` | `pub(crate)` |
| `space_manager_refresh_view` | `src/space_manager.c:113` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_refresh_view(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_mark_view_invalid` | `src/space_manager.c:122` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_mark_view_invalid(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_untile_window` | `src/space_manager.c:130` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState` | `pub(crate) fn space_manager_untile_window(space_manager: &mut SpaceManager, space_id: SpaceId, window_id: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState)` | `pub(crate)` |
| `space_manager_get_label_for_space` | `src/space_manager.c:145` | event loop | `SpaceManager` | `pub(crate) fn space_manager_get_label_for_space(space_manager: &mut SpaceManager, space_id: SpaceId) -> Option<&mut SpaceLabel>` | `pub(crate)` |
| `space_manager_get_space_for_label` | `src/space_manager.c:157` | event loop | `SpaceManager` | `pub(crate) fn space_manager_get_space_for_label<'space_manager>(space_manager: &'space_manager mut SpaceManager, label: &[u8]) -> Option<&'space_manager mut SpaceLabel>` | `pub(crate)` |
| `space_manager_remove_label_for_space` | `src/space_manager.c:169` | event loop | `SpaceManager` | `pub(crate) fn space_manager_remove_label_for_space(space_manager: &mut SpaceManager, space_id: SpaceId) -> bool` | `pub(crate)` |
| `space_manager_set_label_for_space` | `src/space_manager.c:183` | event loop | `SpaceManager` | `pub(crate) fn space_manager_set_label_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, label: String)` | `pub(crate)` |
| `space_manager_set_layout_for_space` | `src/space_manager.c:202` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState` | `pub(crate) fn space_manager_set_layout_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, view_type: ViewType, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState)` | `pub(crate)` |
| `space_manager_set_gap_for_space` | `src/space_manager.c:213` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_set_gap_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, type_of_change: i32, gap: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `space_manager_toggle_gap_for_space` | `src/space_manager.c:230` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_toggle_gap_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `space_manager_toggle_mission_control` | `src/space_manager.c:247` | event loop | `WindowManager`, `MissionControlMode` | `pub(crate) fn space_manager_toggle_mission_control(space_id: SpaceId, window_manager: &mut WindowManager, mission_control_mode: &mut MissionControlMode)` | `pub(crate)` |
| `space_manager_toggle_show_desktop` | `src/space_manager.c:253` | event loop | `WindowManager`, `MissionControlMode` | `pub(crate) fn space_manager_toggle_show_desktop(space_id: SpaceId, window_manager: &mut WindowManager, mission_control_mode: &mut MissionControlMode)` | `pub(crate)` |
| `space_manager_set_layout_for_all_spaces` | `src/space_manager.c:259` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState` | `pub(crate) fn space_manager_set_layout_for_all_spaces(space_manager: &mut SpaceManager, layout: ViewType, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState)` | `pub(crate)` |
| `space_manager_set_window_gap_for_all_spaces` | `src/space_manager.c:276` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_set_window_gap_for_all_spaces(space_manager: &mut SpaceManager, window_gap: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_set_top_padding_for_all_spaces` | `src/space_manager.c:288` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_set_top_padding_for_all_spaces(space_manager: &mut SpaceManager, top_padding: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_set_bottom_padding_for_all_spaces` | `src/space_manager.c:300` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_set_bottom_padding_for_all_spaces(space_manager: &mut SpaceManager, bottom_padding: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_set_left_padding_for_all_spaces` | `src/space_manager.c:312` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_set_left_padding_for_all_spaces(space_manager: &mut SpaceManager, left_padding: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_set_right_padding_for_all_spaces` | `src/space_manager.c:324` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_set_right_padding_for_all_spaces(space_manager: &mut SpaceManager, right_padding: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_set_split_type_for_all_spaces` | `src/space_manager.c:336` | event loop | `SpaceManager` | `pub(crate) fn space_manager_set_split_type_for_all_spaces(space_manager: &mut SpaceManager, split_type: WindowNodeSplit)` | `pub(crate)` |
| `space_manager_set_auto_balance_for_all_spaces` | `src/space_manager.c:346` | event loop | `SpaceManager` | `pub(crate) fn space_manager_set_auto_balance_for_all_spaces(space_manager: &mut SpaceManager, auto_balance: u32)` | `pub(crate)` |
| `space_manager_set_padding_for_space` | `src/space_manager.c:356` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_set_padding_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, type_of_change: i32, top: i32, bottom: i32, left: i32, right: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `space_manager_toggle_padding_for_space` | `src/space_manager.c:379` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_toggle_padding_for_space(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `space_manager_rotate_space` | `src/space_manager.c:396` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_rotate_space(space_manager: &mut SpaceManager, space_id: SpaceId, degrees: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `space_manager_mirror_space` | `src/space_manager.c:408` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_mirror_space(space_manager: &mut SpaceManager, space_id: SpaceId, axis: WindowNodeSplit, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `space_manager_equalize_space` | `src/space_manager.c:420` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_equalize_space(space_manager: &mut SpaceManager, space_id: SpaceId, axis_flag: u32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `space_manager_balance_space` | `src/space_manager.c:432` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_balance_space(space_manager: &mut SpaceManager, space_id: SpaceId, axis_flag: u32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `space_manager_tile_window_on_space_with_insertion_point` | `src/space_manager.c:444` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_tile_window_on_space_with_insertion_point(space_manager: &mut SpaceManager, window_id: WindowId, space_id: SpaceId, insertion_point: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> SpaceId` | `pub(crate)` |
| `space_manager_tile_window_on_space` | `src/space_manager.c:462` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_tile_window_on_space(space_manager: &mut SpaceManager, window_id: WindowId, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> SpaceId` | `pub(crate)` |
| `space_manager_toggle_window_split` | `src/space_manager.c:467` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_toggle_window_split(space_manager: &mut SpaceManager, window_id: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_mission_control_index` | `src/space_manager.c:491` | event loop | — | `pub(crate) fn space_manager_mission_control_index(space_id: SpaceId) -> i32` | `pub(crate)` |
| `space_manager_mission_control_space` | `src/space_manager.c:521` | event loop | — | `pub(crate) fn space_manager_mission_control_space(desktop_id: i32) -> SpaceId` | `pub(crate)` |
| `space_manager_cursor_space` | `src/space_manager.c:551` | event loop | — | `pub(crate) fn space_manager_cursor_space() -> SpaceId` | `pub(crate)` |
| `space_manager_prev_space` | `src/space_manager.c:557` | event loop | — | `pub(crate) fn space_manager_prev_space(space_id: SpaceId) -> SpaceId` | `pub(crate)` |
| `space_manager_next_space` | `src/space_manager.c:586` | event loop | — | `pub(crate) fn space_manager_next_space(space_id: SpaceId) -> SpaceId` | `pub(crate)` |
| `space_manager_first_space` | `src/space_manager.c:615` | event loop | — | `pub(crate) fn space_manager_first_space() -> SpaceId` | `pub(crate)` |
| `space_manager_last_space` | `src/space_manager.c:633` | event loop | — | `pub(crate) fn space_manager_last_space() -> SpaceId` | `pub(crate)` |
| `space_manager_active_space` | `src/space_manager.c:653` | event loop; also start-up on main through `space_manager_begin` | `WindowManager` | `pub(crate) fn space_manager_active_space(window_manager: &mut WindowManager) -> SpaceId` | `pub(crate)` |
| `space_manager_move_window_list_to_space` | `src/space_manager.c:665` | event loop | — | `fn space_manager_move_window_list_to_space(space_id: SpaceId, window_list: &[WindowId])` | private |
| `space_manager_move_window_to_space` | `src/space_manager.c:686` | event loop | — | `pub(crate) fn space_manager_move_window_to_space(space_id: SpaceId, window_id: WindowId)` | `pub(crate)` |
| `space_manager_find_first_user_space_for_display` | `src/space_manager.c:707` | event loop | — | `fn space_manager_find_first_user_space_for_display(display_id: DisplayId) -> SpaceId` | private |
| `space_manager_is_space_last_user_space` | `src/space_manager.c:724` | event loop | — | `fn space_manager_is_space_last_user_space(space_id: SpaceId) -> bool` | private |
| `space_manager_swap_space_with_space_on_display` | `src/space_manager.c:745` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `fn space_manager_swap_space_with_space_on_display(a_display_id: DisplayId, a_space_id: SpaceId, b_display_id: DisplayId, b_space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> SpaceOpError` | private |
| `space_manager_swap_space_with_space` | `src/space_manager.c:801` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MissionControlMode` | `pub(crate) fn space_manager_swap_space_with_space(acting_space_id: SpaceId, selector_space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mission_control_mode: &mut MissionControlMode) -> SpaceOpError` | `pub(crate)` |
| `space_manager_move_space_to_space` | `src/space_manager.c:851` | event loop | `WindowManager`, `MissionControlMode` | `pub(crate) fn space_manager_move_space_to_space(acting_space_id: SpaceId, selector_space_id: SpaceId, window_manager: &mut WindowManager, mission_control_mode: &mut MissionControlMode) -> SpaceOpError` | `pub(crate)` |
| `space_manager_move_space_to_display` | `src/space_manager.c:891` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MissionControlMode` | `pub(crate) fn space_manager_move_space_to_display(space_manager: &mut SpaceManager, space_id: SpaceId, display_id: DisplayId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mission_control_mode: &mut MissionControlMode) -> SpaceOpError` | `pub(crate)` |
| `space_manager_focus_space_using_gesture` | `src/space_manager.c:927` | event loop | `WindowManager` | `pub(crate) fn space_manager_focus_space_using_gesture(new_display_id: DisplayId, new_space_id: SpaceId, window_manager: &mut WindowManager) -> bool` | `pub(crate)` |
| `space_manager_focus_space` | `src/space_manager.c:985` | event loop | `WindowManager`, `MissionControlMode` | `pub(crate) fn space_manager_focus_space(space_id: SpaceId, window_manager: &mut WindowManager, mission_control_mode: &mut MissionControlMode) -> SpaceOpError` | `pub(crate)` |
| `space_manager_switch_space` | `src/space_manager.c:1011` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MissionControlMode` | `pub(crate) fn space_manager_switch_space(space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mission_control_mode: &mut MissionControlMode) -> SpaceOpError` | `pub(crate)` |
| `space_manager_destroy_space` | `src/space_manager.c:1037` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState`, `MissionControlMode` | `pub(crate) fn space_manager_destroy_space(space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> SpaceOpError` | `pub(crate)` |
| `space_manager_add_space` | `src/space_manager.c:1062` | event loop | `MissionControlMode` | `pub(crate) fn space_manager_add_space(space_id: SpaceId, mission_control_mode: &mut MissionControlMode) -> SpaceOpError` | `pub(crate)` |
| `space_manager_assign_process_to_space` | `src/space_manager.c:1074` | — | — | *not translated — see the second table* | — |
| `space_manager_assign_process_to_all_spaces` | `src/space_manager.c:1079` | — | — | *not translated — see the second table* | — |
| `space_manager_is_window_on_active_space` | `src/space_manager.c:1084` | — | — | *not translated — see the second table* | — |
| `space_manager_is_window_on_space` | `src/space_manager.c:1091` | event loop | — | `pub(crate) fn space_manager_is_window_on_space(space_id: SpaceId, window_id: WindowId) -> bool` | `pub(crate)` |
| `space_manager_mark_spaces_invalid_for_display` | `src/space_manager.c:1106` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_mark_spaces_invalid_for_display(space_manager: &mut SpaceManager, display_id: DisplayId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_mark_spaces_invalid` | `src/space_manager.c:1122` | event loop | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_mark_spaces_invalid(space_manager: &mut SpaceManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_refresh_application_windows` | `src/space_manager.c:1133` | event loop | `ProcessManager`, `DisplayManager`, `WindowManager`, `SpaceManager`, `MouseDragState`, `MissionControlMode` | `pub(crate) fn space_manager_refresh_application_windows(space_manager: &mut SpaceManager, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> bool` | `pub(crate)` |
| `space_manager_handle_display_add` | `src/space_manager.c:1150` | event loop | `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_handle_display_add(space_manager: &mut SpaceManager, display_id: DisplayId, window_manager: &mut WindowManager)` | `pub(crate)` |
| `space_manager_begin` | `src/space_manager.c:1202` | start-up only (main thread, `src/yabai.c:337`) | `DisplayManager`, `WindowManager`, `SpaceManager` | `pub(crate) fn space_manager_begin(space_manager: &mut SpaceManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | `pub(crate)` |

## `src/space.c`

| C name | file:line | thread context | event-loop-owned managers touched transitively | Rust signature | visibility |
| --- | --- | --- | --- | --- | --- |
| `space_display_id` | `src/space.c:3` | event loop; also start-up on main through `space_manager_begin` → `view_create` → `view_update` (`view.c:970`) | — | `pub(crate) fn space_display_id(space_id: SpaceId) -> DisplayId` | `pub(crate)` |
| `space_window_list_for_connection` | `src/space.c:17` | event loop | `WindowManager` | `pub(crate) fn space_window_list_for_connection(space_list: &[SpaceId], connection_id: i32, include_minimized: bool, window_manager: &mut WindowManager) -> Vec<WindowId>` | `pub(crate)` |
| `space_window_list` | `src/space.c:83` | event loop | `WindowManager` | `pub(crate) fn space_window_list(space_id: SpaceId, include_minimized: bool, window_manager: &mut WindowManager) -> Vec<WindowId>` | `pub(crate)` |
| `space_is_user` | `src/space.c:88` | event loop; also start-up on main through `space_manager_begin` → `view_create` (`view.c:1000`) | — | `pub(crate) fn space_is_user(space_id: SpaceId) -> bool` | `pub(crate)` |
| `space_is_fullscreen` | `src/space.c:93` | event loop | — | `pub(crate) fn space_is_fullscreen(space_id: SpaceId) -> bool` | `pub(crate)` |
| `space_is_system` | `src/space.c:98` | — | — | *not translated — see the second table* | — |
| `space_is_visible` | `src/space.c:103` | event loop | — | `pub(crate) fn space_is_visible(space_id: SpaceId) -> bool` | `pub(crate)` |

## Not translated

`DECISIONS.md` 5: dead code is not translated, and each removal is one line in `DEVIATIONS.md`.

| C name | file:line | why |
| --- | --- | --- |
| `compare_view` | `src/space_manager.c:9` | the `table_compare_func` disappears into `K: PartialEq` (`patterns/state-and-ownership.md` §1.3 and §6.1: *"compare vanishes"*). `hash_view` survives as `hash_view_key`, the `hash: fn(&K) -> u64` field of `Table<SpaceId, View>` |
| `space_manager_assign_process_to_space` | `src/space_manager.c:1074` | defined and declared (`space_manager.h:101`), called from nowhere — verified by grep over all of `src/` |
| `space_manager_assign_process_to_all_spaces` | `src/space_manager.c:1079` | same; `space_manager.h:102` is its only other mention |
| `space_manager_is_window_on_active_space` | `src/space_manager.c:1084` | same; `space_manager.h:103` is its only other mention |
| `space_is_system` | `src/space.c:98` | same; `space.h:8` is its only other mention. `space_is_user` and `space_is_fullscreen`, its two neighbours over the identical `SLSSpaceGetType` call, are both live |

The four dead functions match `files/space-and-display.md:1611-1614`, which lists exactly these
four as "referenced only by its own header".

---

## Calls this module makes into other modules

The shapes this module expects to pass. Each is derived with the same §2 rule from the callee's
own C signature and `state-access.tsv` row; the owning module's `state-access/*.md` is
authoritative and must agree parameter for parameter.

### `crate::view`

| call site | C call | expected Rust call shape |
| --- | --- | --- |
| `space_manager.c:21`, `:39`, `:60`, `:86` | `view_serialize(rsp, view, flags)` | `view_serialize(response: &mut Response, space_manager: &mut SpaceManager, space_id: SpaceId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` — the `(space_manager, space_id)` pair in place of `struct view *` is spelled out verbatim in `patterns/state-and-ownership.md` §2.2 step 2 |
| `space_manager.c:107`, `:1225` | `view_create(sid)` | `view_create(space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager) -> SpaceId` — returns the `View` by value, because `SpaceManager::view` is `Table<SpaceId, View>` (§3.2); non-nullable (`patterns/idioms-and-conventions.md` §8.4) |
| `space_manager.c:118`, `:224`, `:241`, `:282`, `:294`, `:306`, `:318`, `:330`, `:373`, `:390`, `:402`, `:414`, `:426`, `:438`, `:478`, `:790`, `:791` | `view_update(view)` | `view_update(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` |
| `space_manager.c:119`, `:225`, `:242`, `:283`, `:295`, `:307`, `:319`, `:331`, `:374`, `:391`, `:403`, `:415`, `:427`, `:439`, `:479`, `:793`, `:794` | `view_flush(view)` | `view_flush(space_manager: &mut SpaceManager, space_id: SpaceId, window_manager: &mut WindowManager)` |
| `space_manager.c:206`, `:266` | `view_clear(view)` | `view_clear(space_manager: &mut SpaceManager, space_id: SpaceId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState)` |
| `space_manager.c:135` | `view_remove_window_node(view, window)` | `view_remove_window_node(space_manager: &mut SpaceManager, space_id: SpaceId, window_id: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, mouse_drag_state: &mut MouseDragState) -> Option<NodeId>` |
| `space_manager.c:450` | `view_add_window_node_with_insertion_point(view, window, insertion_point)` | `view_add_window_node_with_insertion_point(space_manager: &mut SpaceManager, space_id: SpaceId, window_id: WindowId, insertion_point: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> Option<NodeId>` — never `None`; the C `assert(node)` at `:451` becomes `debug_assert!` on nothing, so the callee returns a bare `NodeId` |
| `space_manager.c:472` | `view_find_window_node(view, window->id)` | `view_find_window_node(space_manager: &mut SpaceManager, space_id: SpaceId, window_id: WindowId) -> Option<NodeId>` |
| `space_manager.c:139`, `:454`, `:483` | `window_node_flush(node)` | `window_node_flush(space_id: SpaceId, node_id: NodeId, window_manager: &mut WindowManager, space_manager: &mut SpaceManager)` — the `(SpaceId, NodeId)` pair in place, managers appended, exactly like `insert_feedback_show` in §2.3 example 1 |
| `space_manager.c:401` | `window_node_rotate(view->root, degrees)` | `window_node_rotate(space_id: SpaceId, node_id: NodeId, degrees: i32, space_manager: &mut SpaceManager)`, called with `ROOT_NODE_ID` |
| `space_manager.c:413` | `window_node_mirror(view->root, axis)` | `window_node_mirror(space_id: SpaceId, node_id: NodeId, axis: WindowNodeSplit, space_manager: &mut SpaceManager) -> NodeId` |
| `space_manager.c:425` | `window_node_equalize(view->root, axis_flag)` | `window_node_equalize(space_id: SpaceId, node_id: NodeId, axis_flag: u32, space_manager: &mut SpaceManager)` |
| `space_manager.c:437`, `:477` | `window_node_balance(view->root, axis_flag)` | `window_node_balance(space_id: SpaceId, node_id: NodeId, axis_flag: u32, space_manager: &mut SpaceManager) -> BalanceNode` |
| `space_manager.c:481` | `window_node_update(view, node->parent)` | `window_node_update(space_manager: &mut SpaceManager, space_id: SpaceId, node_id: NodeId, window_manager: &mut WindowManager)` — see judgement call 7 |
| `space_manager.c:473` | `window_node_is_intermediate(node)` | `window_node_is_intermediate(space_id: SpaceId, node_id: NodeId, space_manager: &mut SpaceManager) -> bool` |
| `space_manager.c:116`, `:125`, `:132`, `:208`, `:216`, `:233`, `:263`, `:280`, `:292`, `:304`, `:316`, `:328`, `:340`, `:350`, `:359`, `:382`, `:384`, `:399`, `:411`, `:423`, `:435`, `:447`, `:470`, `:476` | `view->layout`, `view_check_flag`, `view_set_flag`, `view_clear_flag` | field reads and `ViewFlag` tests on the `View` resolved out of `SpaceManager::view`; no cross-module function call after `patterns/idioms-and-conventions.md` turns the three flag helpers into `&View` / `&mut View` helpers |

### `crate::window_manager`

| call site | C call | expected Rust call shape |
| --- | --- | --- |
| `space_manager.c:134`, `:449` | `window_manager_adjust_layer(window, layer)` | `window_manager_adjust_layer(window_id: WindowId, layer: i32, window_manager: &mut WindowManager)` — `state-access.tsv` shows no global for this one, but the C reads `window->layer` (`window_manager.c:822`), so the `WindowId` is **resolved** and `WindowManager` is in `Direct` by §2.1 |
| `space_manager.c:209`, `:269`, `:1056` | `window_manager_validate_and_check_for_windows_on_space(sm, wm, sid)` | `window_manager_validate_and_check_for_windows_on_space(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, space_id: SpaceId, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` — both declared managers keep their C positions, `display_manager` is appended |
| `space_manager.c:656` | `window_manager_focused_window(&g_window_manager)` | `window_manager_focused_window(window_manager: &mut WindowManager) -> Option<WindowId>` |
| `space_manager.c:1141` | `window_manager_add_existing_application_windows(sm, &g_window_manager, application, i)` | `window_manager_add_existing_application_windows(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, process_id: ProcessId, refresh_index: i32, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> bool` |
| `space.c:45`, `:58` | `window_manager_find_window(&g_window_manager, wid)` | `window_manager_find_window(window_manager: &mut WindowManager, window_id: WindowId) -> Option<WindowId>` (`THREADS.md:1257`) |
| `space_manager.c:1135`, `:1137`, `:1139`, `:1147` | `g_window_manager.applications_to_refresh`, `g_window_manager.window.count` | `window_manager.applications_to_refresh: Vec<ProcessId>` (§3.2) and `window_manager.window.len()` (`Table::len` → `i32`, §6.2) |
| `space_manager.c:750`, `:751`, `:797` | `g_window_manager.window_animation_duration` | field read/write on `&mut WindowManager` |

### `crate::window`

| call site | C call | expected Rust call shape |
| --- | --- | --- |
| `space_manager.c:31`, `:1094` | `window_space_list(window->id, &space_count)` | `window_space_list(window_id: WindowId) -> Vec<SpaceId>` — the `NULL` return **is** load-bearing: `space_manager_query_spaces_for_window` returns `false` and prints nothing on `NULL`, but would print `"[\n"` and return `true` for an empty list |
| `space_manager.c:469` | `window_space(window->id)` | `window_space(window_id: WindowId) -> SpaceId` |
| `space_manager.c:658` | `window_display_id(window->id)` | `window_display_id(window_id: WindowId) -> DisplayId` |
| `space.c:59` | `window_check_flag(window, WINDOW_MINIMIZE)` | `window_check_flag(window: &Window, flag: WindowFlag) -> bool` (`patterns/idioms-and-conventions.md:780`) |

### `crate::display` and `crate::display_manager`

| call site | C call | expected Rust call shape |
| --- | --- | --- |
| `space_manager.c:52`, `:79`, `:710`, `:729`, `:1109`, `:1153`, `:1221` | `display_space_list(did, &count)` | `display_space_list(display_id: DisplayId) -> Option<Vec<SpaceId>>` (`patterns/state-and-ownership.md:913`, `:1520`) |
| `space_manager.c:73`, `:1125`, `:1216` | `display_manager_active_display_list(&count)` | `display_manager_active_display_list() -> Vec<DisplayId>` — `display_manager.c:398-403` always returns the `ts_alloc_list` block, so there is no `NULL` to model; the `space_manager.c:74` guard becomes `is_empty()` |
| `space_manager.c:554`, `:662`, `:909`, `:929`, `:1112`, and `space.c:105` | `display_space_id(did)` | `display_space_id(display_id: DisplayId) -> SpaceId` |
| `space_manager.c:553`, `:939` | `display_manager_cursor_display_id()` | `display_manager_cursor_display_id() -> DisplayId` |
| `space_manager.c:659` | `display_manager_active_display_id()` | `display_manager_active_display_id() -> DisplayId` |
| `space_manager.c:747`, `:748`, `:812`, `:862`, `:900`, `:906`, `:997`, `:1022`, `:1025`, `:1049`, `:1068` | `display_manager_display_is_animating(did)` | `display_manager_display_is_animating(display_id: DisplayId) -> bool` |
| `space_manager.c:934`, `:1002`, `:1030` | `display_manager_focus_display(did, sid)` | `display_manager_focus_display(display_id: DisplayId, space_id: SpaceId, window_manager: &mut WindowManager)` — worked example 5 of `patterns/state-and-ownership.md` §2.3, verbatim |
| `space_manager.c:974` | `display_manager_set_active_display_id(new_did)` | `display_manager_set_active_display_id(display_id: DisplayId)` |
| `space_manager.c:938` | `display_center(did)` | `display_center(display_id: DisplayId) -> CGPoint` |

### `crate::mission_control`, `crate::sa`, `crate::workspace`

| call site | C call | expected Rust call shape |
| --- | --- | --- |
| `space_manager.c:803`, `:853`, `:893`, `:987`, `:1013`, `:1039`, `:1064` | `mission_control_is_active()` | `mission_control_is_active(mission_control_mode: &mut MissionControlMode) -> bool` — see judgement call 1 |
| `space_manager.c:679` | `scripting_addition_move_window_list_to_space(sid, window_list, window_count)` | `scripting_addition_move_window_list_to_space(space_id: SpaceId, window_list: &[WindowId]) -> bool` |
| `space_manager.c:700` | `scripting_addition_move_window_to_space(sid, window->id)` | `scripting_addition_move_window_to_space(space_id: SpaceId, window_id: WindowId) -> bool` |
| `space_manager.c:829`, `:831`, `:833`, `:834`, `:836`, `:837`, `:840`, `:841`, `:843`, `:844`, `:876`, `:878`, `:879`, `:882`, `:884` | `scripting_addition_move_space_after_space(src_sid, dst_sid, focus)` | `scripting_addition_move_space_after_space(source_space_id: SpaceId, destination_space_id: SpaceId, focus: bool) -> bool` |
| `space_manager.c:914` | `scripting_addition_move_space_to_display(src_sid, dst_sid, src_prev_sid, focus)` | `scripting_addition_move_space_to_display(source_space_id: SpaceId, destination_space_id: SpaceId, source_previous_space_id: SpaceId, focus: bool) -> bool` — the C passes `focus_space ? 1 : 0` into a `bool`, so the Rust passes `focus_space` |
| `space_manager.c:1000`, `:1034` | `scripting_addition_focus_space(sid)` | `scripting_addition_focus_space(space_id: SpaceId) -> bool` |
| `space_manager.c:1052` | `scripting_addition_destroy_space(sid)` | `scripting_addition_destroy_space(space_id: SpaceId) -> bool` |
| `space_manager.c:1071` | `scripting_addition_create_space(sid)` | `scripting_addition_create_space(space_id: SpaceId) -> bool` |
| `space_manager.c:675`, `:696` | `workspace_use_macos_space_workaround()` | `workspace_use_macos_space_workaround() -> bool` |

### `crate::misc`

`cfarray_of_cfnumbers(values, element_size, count, number_type)` (`space_manager.c:668`, `:676`,
`:689`, `:697`, `space.c:24`), `add_and_clamp_to_zero(value, delta)` (`space_manager.c:221`,
`:367-370`) and `string_equals(first, second)` (`space_manager.c:161`, `:189`) — none takes a
manager. `buf_len` / `buf_push` / `buf_del` on `sm->labels` become `Vec::len` / `Vec::push` /
`Vec::swap_remove`, and `table_init` / `table_add` / `table_find` / `table_remove` / `table_for`
become `Table::new` / `add` / `find` / `find_mut` / `remove` / `iter` (`patterns/state-and-ownership.md`
§6.2).

---

## Judgement calls

1. **`mission_control_mode` is appended last, after `mouse_drag_state`.** Settled across all
   nine documents by the cross-module pass, against this file's first reading.
   `patterns/state-and-ownership.md` §2.2 step 3 enumerates five managers because §2.1 defines
   `Managers(f)` over managers; `TRANSLATION_PLAN.md` §3.2 — the spec for this wave — names
   `g_mission_control_mode` among the eight event-loop-owned globals to scan for and fixes the
   append order as the `EventLoopOwnedState` field order, where `mission_control_mode`
   (`yabai.c:37`) comes last. `DECISIONS.md` 13 forbids reaching it through a global. The ten rows
   it lands on here are `space_manager_toggle_mission_control`, `_toggle_show_desktop`,
   `_swap_space_with_space`, `_move_space_to_space`, `_move_space_to_display`, `_focus_space`,
   `_switch_space`, `_destroy_space`, `_add_space` and `_refresh_application_windows`.
2. **`signal_event` and `signal_storage` never become parameters here**, not because §2.2 step 3
   omits them — `state-access/event_loop.md` §1 appends both exactly like managers — but because
   no function in this module reaches either: `event_signal_push` is called from
   `src/event_loop.c` alone, and `state-access.tsv` lists neither global in the transitive set of
   `space_manager_refresh_application_windows` or of any other row in this file.
3. **`FILE *rsp` is `&mut Response`, not `&mut dyn std::io::Write`.** `patterns/state-and-ownership.md`
   §2.2 says `&mut dyn std::io::Write`; `GLOSSARY.md` §10.1 and §10.4 say
   `response: &mut Response`, `patterns/message-and-serialisation.md` §7 defines that type, and
   `GLOSSARY.md`'s stated precedence puts it above `patterns/`.
4. **Visibility follows the assignment's test, not `TRANSLATION_PLAN.md` §3.3 step 4.** The
   assignment says `pub(crate)` unless the tsv shows no caller outside the file;
   `TRANSLATION_PLAN.md` says private only where there is *exactly one* caller in the same module.
   `space_manager_query_view` has four in-file callers and would be `pub(crate)` under the plan's
   test, private under the assignment's. Private is used.
5. **`space_manager_find_view` returns a bare `SpaceId`, not `Option<SpaceId>`.**
   `patterns/idioms-and-conventions.md` §8.4 names it as one of the two lookups that are *not*
   nullable, and forbids adding a `None` branch. `space_manager_query_view` keeps its `Option`
   (`space_manager.c:100` returns `table_find`'s result), and so does the loop `continue` at
   `:37`, `:58`, `:84` that `patterns/message-and-serialisation.md` §6 insists must keep skipping
   the JSON separator. `space_manager_tile_window_on_space*` also returns a bare `SpaceId`: it
   returns `space_manager_find_view`'s view on both paths.
6. **A `struct window *` parameter contributes `&mut WindowManager` only where the C body reads a
   field other than `window->id`.** §2.1 says a handle that `f` only passes along contributes
   nothing. `space_manager_move_window_to_space` (`:686`) and `space_manager_is_window_on_space`
   (`:1091`) read nothing but `window->id`, so they take a bare `WindowId` and no manager.
   `space_manager_query_spaces_for_window` (`:26`) and `space_manager_toggle_window_split`
   (`:467`) are the same, but their callees put `WindowManager` in the set anyway.
7. **A `struct window_node *` that sits beside a `struct view *` of the same view contributes only
   its `NodeId`.** `window_node_update(view, node->parent)` (`space_manager.c:481`) would
   otherwise carry two `SpaceId`s. §3.1 already says node comparisons *inside one tree* use a bare
   `NodeId`; the same applies to a node parameter whose view is named beside it. Where the node is
   the only handle — `window_node_flush`, `window_node_rotate`, `window_node_mirror`,
   `window_node_equalize`, `window_node_balance`, `window_node_is_intermediate` — the pair
   `(SpaceId, NodeId)` is passed, as in §2.3 example 1.
8. **`space_manager_get_label_for_space` and `space_manager_get_space_for_label` return a borrow,
   not a handle.** `struct space_label` is not one of the five record kinds `DECISIONS.md` 14
   names, and §3 turns a pointer into a handle only when it *outlives a single expression*;
   neither of the three call sites does (`space_manager.c:1184-1185`, `view.c:892`,
   `message.c:851-854`). `space_manager_get_space_for_label` needs an explicit lifetime because
   `label: &str` is a second input reference and elision cannot pick one; the name
   `'space_manager` follows the `'message` convention of
   `patterns/message-and-serialisation.md` §2.3.
9. **`space_manager_set_layout_for_space`'s third parameter is `view_type`, not `layout`.** The C
   *definition* (`space_manager.c:202`) names it `layout` while the *declaration*
   (`space_manager.h:74`) names it `type`; `GLOSSARY.md` §11 resolves the keyword collision for
   that exact site with `view_type`, and the glossary is binding.
   `space_manager_set_layout_for_all_spaces` keeps `layout`, because the C never calls it `type`.
   `int type` in `_set_gap_for_space` and `_set_padding_for_space` becomes `type_of_change`, also
   from §11.
10. **`hash_view_key` takes `&SpaceId`, not `&u64`.** §1.3 writes `fn hash_view_key(key: &u64)`,
    but §6.1 declares the field as `hash: fn(&K) -> u64` and §3.2 fixes `K = SpaceId` for
    `SpaceManager::view`. `&SpaceId` is the only spelling that compiles at
    `Table::new(23, hash_view_key)` (`space_manager.c:1213`).
11. **`space_window_list*` return a bare `Vec<WindowId>`; `window_space_list`,
    `display_space_list` and `display_manager_active_display_list` return `Option<Vec<…>>`.**
    `patterns/state-and-ownership.md` §7 allows `Option<Vec<T>>` "where the C's NULL return is
    load-bearing". For `space_window_list` it is not: every caller either `return`s on `NULL` and
    would also produce nothing from an empty list (`view.c:587`, `window_manager.c:915`), or
    guards a loop that runs zero times either way (`event_loop.c:1412`, `window_manager.c:2689`,
    `space_manager.c:776`, `:780`). For the other three it is: `space_manager_query_spaces_for_window`,
    `_for_display` and `_for_displays` return `false` and print **nothing** on `NULL`, but would
    print `"[\n"` and return `true` for an empty list.
12. **`space_manager_begin` keeps its managers although it runs on the main thread.**
    `DECISIONS.md` 20 is about main-thread *callbacks*; `patterns/state-and-ownership.md` §1.4
    step 3 runs `space_manager_begin` and its start-up neighbours "on the main thread against
    `&mut state`". No function in these two files is a callback.
13. **`space_manager_move_window_list_to_space` takes one `&[WindowId]`.** The `(uint32_t
    *window_list, int window_count)` pair is derived from one buffer at both call sites
    (`space_manager.c:777`, `:781`), which is the collapse §2.2 spells out for
    `window_manager_animate_window_list`.

## Deviations this table implies

Each is one line in `DEVIATIONS.md`; they are recorded here because the signature is what forces
them.

* `space.c:26` — `goto err` returns `NULL` **without writing `*count`**, and
  `window_manager.c:2632-2633` then passes that uninitialised count to two functions. Collapsing
  the count into the returned `Vec` removes the uninitialised read (`DECISIONS.md` 4).
* `space.c:29` — the `out` path sets `*count = 0` and returns `NULL`; `view.c:915` reads the count
  with no `NULL` test. Same fix, same line.
* `space_manager.c:760-761` — `table_find` may return `NULL` for either view and both are
  dereferenced unconditionally at `:766-771`. `Table::remove` returning `Option<View>` turns each
  into an early return.

## Rows verified by hand against the C

Every row was read against the C body; these are the ones where `state-access.tsv` and the C
disagree, or where the row is load-bearing for another module.

| row | what the tsv says | what the C says |
| --- | --- | --- |
| `window_manager_adjust_layer` (a callee) | no globals at all | reads `window->layer` and `window->id` (`window_manager.c:822-824`), so the `WindowId` is resolved and `WindowManager` is in `Direct` |
| `space_manager_refresh_application_windows` | `g_process_manager` transitively | confirmed: `→ window_manager_add_existing_application_windows` `→ window_manager_create_and_add_window` (`:1491`) `→ window_manager_apply_rules_to_window` (`:124`) `→ window_manager_send_window_to_space` `→ g_process_manager.finder_psn` (`:2104`). That is the `ProcessManager` struct half, not the `PROCESS_TABLE` static, so the parameter is real |
| `space_manager_refresh_application_windows` | `g_mouse_state` transitively | confirmed: `window_manager_create_and_add_window` calls `window_serialize(stdout, window, 0)` under `g_verbose` (`window_manager.c:1523`, `:1537`), and `window.c:706` reads `g_mouse_state.window` — the **drag** half, so the parameter is `&mut MouseDragState` |
| `space_manager_set_split_type_for_all_spaces`, `_set_auto_balance_for_all_spaces` | no globals | confirmed: the bodies touch `sm` only, no `view_update` / `view_flush` — the only two `_for_all_spaces` functions that do not |
| `space_manager_handle_display_add` | `g_connection`, `g_window_manager` | confirmed: `SLSSpaceCopyName` (`:1168`) and `space_manager_active_space` (`:1198`); `DisplayManager` is genuinely absent, so the row appends `window_manager` only |
| `space_manager_move_space_to_space` | `g_window_manager` only | confirmed: no `view_*` call, no `space_manager_swap_space_with_space_on_display`; `WindowManager` arrives through `space_manager_active_space` alone |
| `space_manager_move_window_list_to_space` | `g_connection` only | confirmed — and it contradicts the prose of `patterns/state-and-ownership.md` §2.3 example 4, which lists this callee as reaching "`SpaceManager` and `WindowManager`". The example's final `Managers` set and signature are still right; they are carried by `space_window_list`, `view_update` and `view_flush` |
| `space_window_list_for_connection` | `g_window_manager` | confirmed: `window_manager_find_window` at `space.c:45` and `:58`, and `window_check_flag(window, WINDOW_MINIMIZE)` at `:59` — a field read, so the lookup is a real resolution |
| `space_manager_swap_space_with_space_on_display` | `g_display_manager`, `g_space_manager`, `g_window_manager` | confirmed against the body and against `patterns/state-and-ownership.md` §2.3 example 4, which gives this signature verbatim; the row reproduces it character for character |
| `space_is_system`, `space_manager_assign_process_to_space`, `_assign_process_to_all_spaces`, `_is_window_on_active_space` | no caller outside the file | confirmed by grep over all of `src/`: the only other mention of each is its own prototype |
