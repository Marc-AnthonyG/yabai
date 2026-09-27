# `src/window_manager.h` + `src/window_manager.c` — final Rust signatures

Work unit `W0b-1`. One row per function defined in the pair, in C source order. Wave 1 pastes the
**Rust signature** column verbatim, prefixed by the **Visibility** column.

Derived by applying `patterns/state-and-ownership.md` §2 (binding) to the C, cross-checked against
`doc/rust-rewrite/state-access.tsv`. The tsv extraction is textual and only tracks `g_*` names, so
it misses the handle-resolution half of §2.1 (a resolved `struct view *` pulls in `SpaceManager`, a
resolved `struct window *` or `struct application *` pulls in `WindowManager`). Every row where the
tsv and the C disagree is called out in §4.

## 0. The rule as applied here

1. C parameters in C order, pointers rewritten to handles (`struct window *` → `WindowId`,
   `struct application *` → `ProcessId`, `struct view *` → `&mut SpaceManager` **plus** the
   `SpaceId`, at the view parameter's own position).
2. A manager the C declares keeps its C position.
3. Every remaining manager of `Managers(f)` is appended after the whole C parameter list, in the
   `src/yabai.c:27-35` order: `process_manager`, `display_manager`, `window_manager`,
   `space_manager`, `mouse_drag_state`.
4. Every manager parameter is `&mut`. Statics are never parameters — `CONNECTION`, `BOOTSTRAP_PORT`,
   the three `LAYER_*_WINDOW_LEVEL`, `VERBOSE`, `PROCESS_TABLE`, `MOUSE_TAP_STATE`, `EVENT_SENDER`,
   `CV_HOST_CLOCK_FREQUENCY` and `WORKSPACE_CONTEXT` are read where they are used.
5. `FILE *rsp` → `response: &mut Response`, first position (`GLOSSARY.md` §10.1, §10.4).
6. An out-parameter count paired with a returned buffer collapses into the returned `Vec`; a
   `(pointer, count)` argument pair derived from one buffer collapses into one slice.
7. Returned record pointer → `Option` of its handle; returned list → `Vec`; `bool` stays `bool`;
   observable sentinels stay integers (`DECISIONS.md` 32 — `INT_MAX` from
   `window_manager_find_rank_of_window_in_list`).

Manager spellings: `process_manager: &mut ProcessManager`, `display_manager: &mut DisplayManager`,
`window_manager: &mut WindowManager`, `space_manager: &mut SpaceManager`,
`mouse_drag_state: &mut MouseDragState`.

## 1. Signatures

| C name | file:line | Thread context | Managers touched (transitive) | Rust signature | Visibility |
| --- | --- | --- | --- | --- | --- |
| `hash_wm` | `window_manager.c:9` | any | — | `fn hash_window_manager_key(key: &u32) -> u64` | private |
| `window_manager_is_window_eligible` | `window_manager.c:19` | event loop | WindowManager | `fn window_manager_is_window_eligible(window_id: WindowId, window_manager: &mut WindowManager) -> bool` | pub(crate) |
| `window_manager_query_window_rules` | `window_manager.c:25` | event loop | DisplayManager, WindowManager | `fn window_manager_query_window_rules(response: &mut Response, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_query_windows_for_spaces` | `window_manager.c:38` | event loop | DisplayManager, WindowManager, SpaceManager, MouseDragState | `fn window_manager_query_windows_for_spaces(response: &mut Response, space_list: &[SpaceId], flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_query_windows_for_display` | `window_manager.c:54` | event loop | DisplayManager, WindowManager, SpaceManager, MouseDragState | `fn window_manager_query_windows_for_display(response: &mut Response, display_id: DisplayId, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_query_windows_for_displays` | `window_manager.c:63` | event loop | DisplayManager, WindowManager, SpaceManager, MouseDragState | `fn window_manager_query_windows_for_displays(response: &mut Response, flags: u64, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_rule_matches_window` | `window_manager.c:91` | event loop | WindowManager | `fn window_manager_rule_matches_window(rule: &Rule, window_id: WindowId, window_title: &str, window_role: &str, window_subrole: &str, window_manager: &mut WindowManager) -> bool` | pub(crate) |
| `window_manager_apply_manage_rule_effects_to_window` | `window_manager.c:108` | event loop | SpaceManager, WindowManager, DisplayManager, MouseDragState | `fn window_manager_apply_manage_rule_effects_to_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, effects: &RuleEffects, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_apply_rule_effects_to_window` | `window_manager.c:119` | event loop | SpaceManager, WindowManager, ProcessManager, DisplayManager, MouseDragState, MissionControlMode | `fn window_manager_apply_rule_effects_to_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, effects: &RuleEffects, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | pub(crate) |
| `window_manager_apply_manage_rules_to_window` | `window_manager.c:171` | event loop | SpaceManager, WindowManager, DisplayManager, MouseDragState | `fn window_manager_apply_manage_rules_to_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, window_title: &str, window_role: &str, window_subrole: &str, one_shot_rules: bool, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_apply_rules_to_window` | `window_manager.c:195` | event loop | SpaceManager, WindowManager, ProcessManager, DisplayManager, MouseDragState, MissionControlMode | `fn window_manager_apply_rules_to_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, window_title: &str, window_role: &str, window_subrole: &str, one_shot_rules: bool, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | pub(crate) |
| `window_manager_set_focus_follows_mouse` | `window_manager.c:219` | event loop | WindowManager | `fn window_manager_set_focus_follows_mouse(window_manager: &mut WindowManager, mode: FfmMode)` | pub(crate) |
| `window_manager_set_window_opacity_enabled` | `window_manager.c:232` | event loop | WindowManager | `fn window_manager_set_window_opacity_enabled(window_manager: &mut WindowManager, enabled: bool)` | pub(crate) |
| `window_manager_center_mouse` | `window_manager.c:242` | event loop | WindowManager | `fn window_manager_center_mouse(window_manager: &mut WindowManager, window_id: WindowId)` | pub(crate) |
| `window_manager_should_manage_window` | `window_manager.c:272` | event loop | WindowManager | `fn window_manager_should_manage_window(window_id: WindowId, window_manager: &mut WindowManager) -> bool` | pub(crate) |
| `window_manager_find_managed_window` | `window_manager.c:283` | event loop | WindowManager | `fn window_manager_find_managed_window(window_manager: &mut WindowManager, window_id: WindowId) -> Option<SpaceId>` | pub(crate) |
| `window_manager_remove_managed_window` | `window_manager.c:288` | event loop | WindowManager | `fn window_manager_remove_managed_window(window_manager: &mut WindowManager, window_id: WindowId)` | pub(crate) |
| `window_manager_add_managed_window` | `window_manager.c:293` | event loop | WindowManager, SpaceManager | `fn window_manager_add_managed_window(window_manager: &mut WindowManager, window_id: WindowId, space_manager: &mut SpaceManager, space_id: SpaceId)` | pub(crate) |
| `window_manager_adjust_window_ratio` | `window_manager.c:300` | event loop | WindowManager, SpaceManager | `fn window_manager_adjust_window_ratio(window_manager: &mut WindowManager, window_id: WindowId, type_of_change: i32, ratio: f32, space_manager: &mut SpaceManager) -> WindowOpError` | pub(crate) |
| `window_manager_move_window_relative` | `window_manager.c:330` | event loop | WindowManager | `fn window_manager_move_window_relative(window_manager: &mut WindowManager, window_id: WindowId, type_of_change: i32, delta_x: f32, delta_y: f32) -> WindowOpError` | pub(crate) |
| `window_manager_resize_window_relative_internal` | `window_manager.c:346` | event loop | WindowManager | `fn window_manager_resize_window_relative_internal(window_id: WindowId, frame: CGRect, direction: i32, delta_x: f32, delta_y: f32, animate: bool, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_resize_window_relative` | `window_manager.c:368` | event loop | WindowManager, DisplayManager, SpaceManager | `fn window_manager_resize_window_relative(window_manager: &mut WindowManager, window_id: WindowId, direction: i32, delta_x: f32, delta_y: f32, animate: bool, display_manager: &mut DisplayManager, space_manager: &mut SpaceManager) -> WindowOpError` | pub(crate) |
| `window_manager_move_window` | `window_manager.c:415` | event loop | WindowManager | `fn window_manager_move_window(window_id: WindowId, x: f32, y: f32, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_resize_window` | `window_manager.c:425` | event loop | WindowManager | `fn window_manager_resize_window(window_id: WindowId, width: f32, height: f32, window_manager: &mut WindowManager)` | private |
| `window_manager_notify_jankyborders` | `window_manager.c:437` | any (event loop + display link) | — | `fn window_manager_notify_jankyborders(animation_list: &[WindowAnimation], event: u32, skip: bool, wait: bool)` | private |
| `window_manager_create_window_proxy` | `window_manager.c:463` | any (event loop + proxy) | — | `fn window_manager_create_window_proxy(animation_connection: i32, alpha: f32, proxy: &mut WindowProxy)` | private |
| `window_manager_destroy_window_proxy` | `window_manager.c:489` | any (event loop + display link) | — | `fn window_manager_destroy_window_proxy(animation_connection: i32, proxy: &mut WindowProxy)` | private |
| `window_manager_build_window_proxy_thread_proc` | `window_manager.c:507` | any (event loop + proxy) | — | `fn window_manager_build_window_proxy_thread_proc(window_animation: &mut WindowAnimation)` | private |
| `window_manager_animate_window_list_thread_proc` | `window_manager.c:537` | display link | — | `unsafe extern "C-unwind" fn window_manager_animate_window_list_thread_proc(link: NonNull<CVDisplayLink>, now: NonNull<CVTimeStamp>, output_time: NonNull<CVTimeStamp>, flags: CVOptionFlags, flags_out: NonNull<CVOptionFlags>, data: *mut c_void) -> CVReturn` | private |
| `window_manager_animate_window_list_async` | `window_manager.c:603` | event loop | WindowManager | `fn window_manager_animate_window_list_async(window_list: &[WindowCapture], window_manager: &mut WindowManager)` | private |
| `window_manager_animate_window_list` | `window_manager.c:705` | event loop | WindowManager | `fn window_manager_animate_window_list(window_list: &[WindowCapture], window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_animate_window` | `window_manager.c:718` | event loop | WindowManager | `fn window_manager_animate_window(capture: WindowCapture, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_set_window_frame` | `window_manager.c:729` | event loop | WindowManager | `fn window_manager_set_window_frame(window_id: WindowId, x: f32, y: f32, width: f32, height: f32, window_manager: &mut WindowManager)` | private |
| `window_manager_set_purify_mode` | `window_manager.c:764` | event loop | WindowManager | `fn window_manager_set_purify_mode(window_manager: &mut WindowManager, mode: PurifyMode)` | pub(crate) |
| `window_manager_set_opacity` | `window_manager.c:774` | event loop | WindowManager | `fn window_manager_set_opacity(window_manager: &mut WindowManager, window_id: WindowId, opacity: f32) -> bool` | pub(crate) |
| `window_manager_set_window_opacity` | `window_manager.c:787` | event loop | WindowManager | `fn window_manager_set_window_opacity(window_manager: &mut WindowManager, window_id: WindowId, opacity: f32)` | pub(crate) |
| `window_manager_set_menubar_opacity` | `window_manager.c:796` | event loop | WindowManager | `fn window_manager_set_menubar_opacity(window_manager: &mut WindowManager, opacity: f32)` | pub(crate) |
| `window_manager_set_active_window_opacity` | `window_manager.c:802` | event loop | WindowManager | `fn window_manager_set_active_window_opacity(window_manager: &mut WindowManager, opacity: f32)` | pub(crate) |
| `window_manager_set_normal_window_opacity` | `window_manager.c:809` | event loop | WindowManager | `fn window_manager_set_normal_window_opacity(window_manager: &mut WindowManager, opacity: f32)` | pub(crate) |
| `window_manager_adjust_layer` | `window_manager.c:820` | event loop | WindowManager | `fn window_manager_adjust_layer(window_id: WindowId, layer: i32, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_set_window_layer` | `window_manager.c:827` | event loop | WindowManager | `fn window_manager_set_window_layer(window_id: WindowId, layer: i32, window_manager: &mut WindowManager) -> bool` | pub(crate) |
| `window_manager_purify_window` | `window_manager.c:877` | event loop | WindowManager | `fn window_manager_purify_window(window_manager: &mut WindowManager, window_id: WindowId)` | pub(crate) |
| `window_manager_find_rank_of_window_in_list` | `window_manager.c:898` | event loop | — | `fn window_manager_find_rank_of_window_in_list(window_id: WindowId, window_list: &[WindowId]) -> i32` | pub(crate) |
| `window_manager_find_window_on_space_by_rank_filtering_window` | `window_manager.c:911` | event loop | WindowManager | `fn window_manager_find_window_on_space_by_rank_filtering_window(window_manager: &mut WindowManager, space_id: SpaceId, rank: i32, filter_window_id: WindowId) -> Option<WindowId>` | pub(crate) |
| `window_manager_window_connection_is_jankyborders` | `window_manager.c:933` | event loop | — | `fn window_manager_window_connection_is_jankyborders(window_connection_id: i32) -> bool` | private |
| `window_manager_find_window_at_point_filtering_window` | `window_manager.c:944` | event loop | WindowManager | `fn window_manager_find_window_at_point_filtering_window(window_manager: &mut WindowManager, point: CGPoint, filter_window_id: WindowId) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_window_at_point` | `window_manager.c:961` | event loop | WindowManager | `fn window_manager_find_window_at_point(window_manager: &mut WindowManager, point: CGPoint) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_window_below_cursor` | `window_manager.c:978` | event loop | WindowManager | `fn window_manager_find_window_below_cursor(window_manager: &mut WindowManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_closest_managed_window_in_direction` | `window_manager.c:985` | event loop | WindowManager, SpaceManager | `fn window_manager_find_closest_managed_window_in_direction(window_manager: &mut WindowManager, window_id: WindowId, direction: i32, space_manager: &mut SpaceManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_prev_managed_window` | `window_manager.c:999` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_prev_managed_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_next_managed_window` | `window_manager.c:1013` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_next_managed_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_first_managed_window` | `window_manager.c:1027` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_first_managed_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_last_managed_window` | `window_manager.c:1038` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_last_managed_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_recent_managed_window` | `window_manager.c:1049` | event loop | WindowManager | `fn window_manager_find_recent_managed_window(window_manager: &mut WindowManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_prev_window_in_stack` | `window_manager.c:1060` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_prev_window_in_stack(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_next_window_in_stack` | `window_manager.c:1077` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_next_window_in_stack(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_first_window_in_stack` | `window_manager.c:1094` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_first_window_in_stack(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_last_window_in_stack` | `window_manager.c:1105` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_last_window_in_stack(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_recent_window_in_stack` | `window_manager.c:1116` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_recent_window_in_stack(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_window_in_stack` | `window_manager.c:1127` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_window_in_stack(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, index: i32, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_largest_managed_window` | `window_manager.c:1138` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_largest_managed_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_smallest_managed_window` | `window_manager.c:1157` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_find_smallest_managed_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, display_manager: &mut DisplayManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_sibling_for_managed_window` | `window_manager.c:1176` | event loop | WindowManager, SpaceManager | `fn window_manager_find_sibling_for_managed_window(window_manager: &mut WindowManager, window_id: WindowId, space_manager: &mut SpaceManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_first_nephew_for_managed_window` | `window_manager.c:1190` | event loop | WindowManager, SpaceManager | `fn window_manager_find_first_nephew_for_managed_window(window_manager: &mut WindowManager, window_id: WindowId, space_manager: &mut SpaceManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_second_nephew_for_managed_window` | `window_manager.c:1204` | event loop | WindowManager, SpaceManager | `fn window_manager_find_second_nephew_for_managed_window(window_manager: &mut WindowManager, window_id: WindowId, space_manager: &mut SpaceManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_uncle_for_managed_window` | `window_manager.c:1218` | event loop | WindowManager, SpaceManager | `fn window_manager_find_uncle_for_managed_window(window_manager: &mut WindowManager, window_id: WindowId, space_manager: &mut SpaceManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_first_cousin_for_managed_window` | `window_manager.c:1235` | event loop | WindowManager, SpaceManager | `fn window_manager_find_first_cousin_for_managed_window(window_manager: &mut WindowManager, window_id: WindowId, space_manager: &mut SpaceManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_second_cousin_for_managed_window` | `window_manager.c:1252` | event loop | WindowManager, SpaceManager | `fn window_manager_find_second_cousin_for_managed_window(window_manager: &mut WindowManager, window_id: WindowId, space_manager: &mut SpaceManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_make_key_window` | `window_manager.c:1269` | event loop | — | `fn window_manager_make_key_window(window_process_serial_number: &ProcessSerialNumber, window_id: WindowId)` | private |
| `window_manager_focus_window_without_raise` | `window_manager.c:1293` | event loop | WindowManager | `fn window_manager_focus_window_without_raise(window_process_serial_number: &ProcessSerialNumber, window_id: WindowId, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_focus_window_with_raise` | `window_manager.c:1324` | event loop | — | `fn window_manager_focus_window_with_raise(window_process_serial_number: &ProcessSerialNumber, window_id: WindowId, window_ref: AXUIElementRef)` | pub(crate) |
| `window_manager_focused_application` | `window_manager.c:1339` | event loop | WindowManager | `fn window_manager_focused_application(window_manager: &mut WindowManager) -> Option<ProcessId>` | private |
| `window_manager_focused_window` | `window_manager.c:1352` | event loop | WindowManager | `fn window_manager_focused_window(window_manager: &mut WindowManager) -> Option<WindowId>` | pub(crate) |
| `window_manager_find_lost_front_switched_event` | `window_manager.c:1364` | event loop | WindowManager | `fn window_manager_find_lost_front_switched_event(window_manager: &mut WindowManager, process_id: ProcessId) -> bool` | pub(crate) |
| `window_manager_remove_lost_front_switched_event` | `window_manager.c:1369` | event loop | WindowManager | `fn window_manager_remove_lost_front_switched_event(window_manager: &mut WindowManager, process_id: ProcessId)` | pub(crate) |
| `window_manager_add_lost_front_switched_event` | `window_manager.c:1374` | event loop | WindowManager | `fn window_manager_add_lost_front_switched_event(window_manager: &mut WindowManager, process_id: ProcessId)` | pub(crate) |
| `window_manager_find_lost_focused_event` | `window_manager.c:1379` | event loop | WindowManager | `fn window_manager_find_lost_focused_event(window_manager: &mut WindowManager, window_id: WindowId) -> bool` | pub(crate) |
| `window_manager_remove_lost_focused_event` | `window_manager.c:1384` | event loop | WindowManager | `fn window_manager_remove_lost_focused_event(window_manager: &mut WindowManager, window_id: WindowId)` | pub(crate) |
| `window_manager_add_lost_focused_event` | `window_manager.c:1389` | event loop | WindowManager | `fn window_manager_add_lost_focused_event(window_manager: &mut WindowManager, window_id: WindowId)` | pub(crate) |
| `window_manager_find_window` | `window_manager.c:1394` | event loop | WindowManager | `fn window_manager_find_window(window_manager: &mut WindowManager, window_id: WindowId) -> Option<WindowId>` | pub(crate) |
| `window_manager_remove_window` | `window_manager.c:1399` | event loop | WindowManager | `fn window_manager_remove_window(window_manager: &mut WindowManager, window_id: WindowId)` | pub(crate) |
| `window_manager_add_window` | `window_manager.c:1404` | event loop | WindowManager | `fn window_manager_add_window(window_manager: &mut WindowManager, window: Window)` | private |
| `window_manager_find_application` | `window_manager.c:1409` | event loop | WindowManager | `fn window_manager_find_application(window_manager: &mut WindowManager, process_id: ProcessId) -> Option<ProcessId>` | pub(crate) |
| `window_manager_remove_application` | `window_manager.c:1414` | event loop | WindowManager | `fn window_manager_remove_application(window_manager: &mut WindowManager, process_id: ProcessId)` | pub(crate) |
| `window_manager_add_application` | `window_manager.c:1419` | event loop | WindowManager | `fn window_manager_add_application(window_manager: &mut WindowManager, application: Application)` | pub(crate) |
| `window_manager_find_application_windows` | `window_manager.c:1424` | event loop | WindowManager | `fn window_manager_find_application_windows(window_manager: &mut WindowManager, process_id: ProcessId) -> Vec<WindowId>` | pub(crate) |
| `window_manager_create_and_add_window` | `window_manager.c:1438` | event loop | SpaceManager, WindowManager, ProcessManager, DisplayManager, MouseDragState, MissionControlMode | `fn window_manager_create_and_add_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, process_id: ProcessId, window_ref: AXUIElementRef, window_id: WindowId, one_shot_rules: bool, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> Option<WindowId>` | pub(crate) |
| `window_manager_add_application_windows` | `window_manager.c:1546` | event loop | SpaceManager, WindowManager, ProcessManager, DisplayManager, MouseDragState, MissionControlMode | `fn window_manager_add_application_windows(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, process_id: ProcessId, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> Vec<WindowId>` | pub(crate) |
| `window_manager_existing_application_window_list` | `window_manager.c:1580` | event loop | WindowManager | `fn window_manager_existing_application_window_list(process_id: Option<ProcessId>, window_manager: &mut WindowManager) -> Vec<WindowId>` | private |
| `window_manager_add_existing_application_windows` | `window_manager.c:1607` | event loop | SpaceManager, WindowManager, ProcessManager, DisplayManager, MouseDragState, MissionControlMode | `fn window_manager_add_existing_application_windows(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, process_id: ProcessId, refresh_index: i32, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode) -> bool` | pub(crate) |
| `window_manager_set_window_insertion` | `window_manager.c:1748` | event loop | SpaceManager, DisplayManager, WindowManager | `fn window_manager_set_window_insertion(space_manager: &mut SpaceManager, window_id: WindowId, direction: i32, display_manager: &mut DisplayManager, window_manager: &mut WindowManager) -> WindowOpError` | pub(crate) |
| `window_manager_stack_window` | `window_manager.c:1799` | event loop | SpaceManager, WindowManager, DisplayManager, MouseDragState | `fn window_manager_stack_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, a_window: WindowId, b_window: WindowId, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState) -> WindowOpError` | pub(crate) |
| `window_manager_warp_window` | `window_manager.c:1832` | event loop | SpaceManager, WindowManager, ProcessManager, DisplayManager, MouseDragState | `fn window_manager_warp_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, a_window: WindowId, b_window: WindowId, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState) -> WindowOpError` | pub(crate) |
| `window_manager_swap_window` | `window_manager.c:1950` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_swap_window(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, a_window: WindowId, b_window: WindowId, display_manager: &mut DisplayManager) -> WindowOpError` | pub(crate) |
| `window_manager_minimize_window` | `window_manager.c:2057` | event loop | WindowManager | `fn window_manager_minimize_window(window_id: WindowId, window_manager: &mut WindowManager) -> WindowOpError` | pub(crate) |
| `window_manager_deminimize_window` | `window_manager.c:2068` | event loop | WindowManager | `fn window_manager_deminimize_window(window_id: WindowId, window_manager: &mut WindowManager) -> WindowOpError` | pub(crate) |
| `window_manager_close_window` | `window_manager.c:2078` | event loop | WindowManager | `fn window_manager_close_window(window_id: WindowId, window_manager: &mut WindowManager) -> bool` | pub(crate) |
| `window_manager_send_window_to_space` | `window_manager.c:2092` | event loop | SpaceManager, WindowManager, ProcessManager, DisplayManager, MouseDragState | `fn window_manager_send_window_to_space(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, destination_space_id: SpaceId, moved_by_rule: bool, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_apply_grid` | `window_manager.c:2124` | event loop | SpaceManager, WindowManager, DisplayManager | `fn window_manager_apply_grid(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, rows: u32, columns: u32, x: u32, y: u32, width: u32, height: u32, display_manager: &mut DisplayManager) -> WindowOpError` | pub(crate) |
| `window_manager_make_window_floating` | `window_manager.c:2181` | event loop | SpaceManager, WindowManager, DisplayManager, MouseDragState | `fn window_manager_make_window_floating(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, should_float: bool, force: bool, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_make_window_sticky` | `window_manager.c:2215` | event loop | SpaceManager, WindowManager, DisplayManager, MouseDragState | `fn window_manager_make_window_sticky(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, should_sticky: bool, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_toggle_window_shadow` | `window_manager.c:2245` | event loop | WindowManager | `fn window_manager_toggle_window_shadow(window_id: WindowId, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_wait_for_native_fullscreen_transition` | `window_manager.c:2259` | event loop | WindowManager | `fn window_manager_wait_for_native_fullscreen_transition(window_id: WindowId, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_toggle_window_native_fullscreen` | `window_manager.c:2296` | event loop | WindowManager | `fn window_manager_toggle_window_native_fullscreen(window_id: WindowId, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_toggle_window_zoom_parent` | `window_manager.c:2326` | event loop | WindowManager, SpaceManager | `fn window_manager_toggle_window_zoom_parent(window_manager: &mut WindowManager, window_id: WindowId, space_manager: &mut SpaceManager)` | pub(crate) |
| `window_manager_toggle_window_zoom_fullscreen` | `window_manager.c:2355` | event loop | WindowManager, SpaceManager | `fn window_manager_toggle_window_zoom_fullscreen(window_manager: &mut WindowManager, window_id: WindowId, space_manager: &mut SpaceManager)` | pub(crate) |
| `window_manager_toggle_window_windowed_fullscreen` | `window_manager.c:2384` | event loop | DisplayManager, WindowManager | `fn window_manager_toggle_window_windowed_fullscreen(window_id: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_toggle_window_expose` | `window_manager.c:2402` | event loop | WindowManager | `fn window_manager_toggle_window_expose(window_id: WindowId, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_toggle_window_pip` | `window_manager.c:2410` | event loop | SpaceManager, DisplayManager, WindowManager | `fn window_manager_toggle_window_pip(space_manager: &mut SpaceManager, window_id: WindowId, display_manager: &mut DisplayManager, window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_find_scratchpad_window` | `window_manager.c:2431` | event loop | WindowManager | `fn window_manager_find_scratchpad_window(window_manager: &mut WindowManager, label: &[u8]) -> Option<WindowId>` | private |
| `window_manager_toggle_scratchpad_window_by_label` | `window_manager.c:2442` | event loop | WindowManager, ProcessManager | `fn window_manager_toggle_scratchpad_window_by_label(window_manager: &mut WindowManager, label: &[u8], process_manager: &mut ProcessManager) -> bool` | pub(crate) |
| `window_manager_toggle_scratchpad_window` | `window_manager.c:2448` | event loop | WindowManager, ProcessManager | `fn window_manager_toggle_scratchpad_window(window_manager: &mut WindowManager, window_id: WindowId, forced_mode: i32, process_manager: &mut ProcessManager) -> bool` | private |
| `window_manager_set_scratchpad_for_window` | `window_manager.c:2492` | event loop | WindowManager, ProcessManager, DisplayManager, SpaceManager, MouseDragState | `fn window_manager_set_scratchpad_for_window(window_manager: &mut WindowManager, window_id: WindowId, label: String, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState) -> bool` | pub(crate) |
| `window_manager_remove_scratchpad_for_window` | `window_manager.c:2508` | event loop | WindowManager, ProcessManager, DisplayManager, SpaceManager, MouseDragState | `fn window_manager_remove_scratchpad_for_window(window_manager: &mut WindowManager, window_id: WindowId, unfloat: bool, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState) -> bool` | pub(crate) |
| `window_manager_scratchpad_recover_windows` | `window_manager.c:2529` | event loop | ProcessManager, DisplayManager, WindowManager, SpaceManager, MouseDragState, MissionControlMode | `fn window_manager_scratchpad_recover_windows(process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, window_manager: &mut WindowManager, space_manager: &mut SpaceManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | pub(crate) |
| `window_manager_validate_windows_on_space` | `window_manager.c:2540` | event loop | WindowManager, SpaceManager, DisplayManager, MouseDragState | `fn window_manager_validate_windows_on_space(window_manager: &mut WindowManager, space_manager: &mut SpaceManager, space_id: SpaceId, window_list: &[WindowId], display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | private |
| `window_manager_check_for_windows_on_space` | `window_manager.c:2579` | event loop | WindowManager, SpaceManager, DisplayManager, MouseDragState | `fn window_manager_check_for_windows_on_space(window_manager: &mut WindowManager, space_manager: &mut SpaceManager, space_id: SpaceId, window_list: &[WindowId], display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | private |
| `window_manager_validate_and_check_for_windows_on_space` | `window_manager.c:2625` | event loop | SpaceManager, WindowManager, DisplayManager, MouseDragState | `fn window_manager_validate_and_check_for_windows_on_space(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, space_id: SpaceId, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_correct_for_mission_control_changes` | `window_manager.c:2650` | event loop | SpaceManager, WindowManager, DisplayManager, MouseDragState | `fn window_manager_correct_for_mission_control_changes(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_handle_display_add_and_remove` | `window_manager.c:2679` | event loop | SpaceManager, WindowManager, DisplayManager, MouseDragState | `fn window_manager_handle_display_add_and_remove(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, display_id: DisplayId, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState)` | pub(crate) |
| `window_manager_init` | `window_manager.c:2709` | start-up only | WindowManager | `fn window_manager_init(window_manager: &mut WindowManager)` | pub(crate) |
| `window_manager_begin` | `window_manager.c:2737` | start-up only | SpaceManager, WindowManager, ProcessManager, DisplayManager, MouseDragState, MissionControlMode | `fn window_manager_begin(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, process_manager: &mut ProcessManager, display_manager: &mut DisplayManager, mouse_drag_state: &mut MouseDragState, mission_control_mode: &mut MissionControlMode)` | pub(crate) |

122 rows. `window_manager.c` defines 123 functions; the 123rd, `compare_wm`, is in §2.

## 2. Not translated

| C name | file:line | Why |
| --- | --- | --- |
| `compare_wm` | `window_manager.c:14` | `DECISIONS.md` 16 and `patterns/state-and-ownership.md` §1.3: `Table<K, V>` drops the comparison function and uses `K: PartialEq`. The seven `table_init` arguments at `window_manager.c:2727-2733` disappear with it |
| `window_manager_tile_window` | declared `window_manager.h:117`, no definition | `DECISIONS.md` 5: a declaration with no definition. No caller anywhere in `src/` either. One `DEVIATIONS.md` line |

`window_manager_focus_window_with_raise` (`window_manager.c:1324`) survives, but its `#else` arm
(`scripting_addition_focus_window(window_id)`, `:1336`) is behind `#if 1` and is not translated —
`DECISIONS.md` 5. One `DEVIATIONS.md` line.

## 3. Thread contexts, per `THREADS.md` §1

Everything in this pair runs on **EVENTLOOP**, with five exceptions.

* `window_manager_animate_window_list_thread_proc` is the CVDisplayLink output callback
  (`THREADS.md` §1.6) — **display link**. It takes no manager (`DECISIONS.md` 20) and its signature
  is `THREADS.md` §2.5 verbatim.
* `window_manager_build_window_proxy_thread_proc` runs on a **PROXY** thread when
  `pthread_create` succeeds (`:666`) and on EVENTLOOP when it fails (`:669`) — **any**.
* `window_manager_notify_jankyborders` is called from EVENTLOOP (`:653`, `:654`, `:689`) and from
  the display-link callback (`:579`) — **any**.
* `window_manager_create_window_proxy` is called from PROXY (`:531`) and EVENTLOOP (`:652`) —
  **any**. `window_manager_destroy_window_proxy` is called from the display-link callback (`:585`)
  and EVENTLOOP (`:663`) — **any**.
* `window_manager_init` (`yabai.c:336`) and `window_manager_begin` (`yabai.c:338`) have no caller
  outside start-up — **start-up only**. Per `patterns/state-and-ownership.md` §1.4 step 3 that pass
  runs on the main thread *against `&mut state`*, before the event-loop thread is spawned, so both
  take managers like any EVENTLOOP function. `DECISIONS.md` 20 constrains main-thread *callbacks*,
  which none of these are.

`window_manager_begin` transitively drags a large part of the module through that start-up
main-thread pass (`window_manager_add_application`, `window_manager_add_existing_application_windows`
and everything below it). This changes no signature and those rows stay **event loop**.

## 4. Where this table departs from `state-access.tsv`

The tsv only records `g_*` identifiers, so it under-reports `Managers(f)` wherever
`patterns/state-and-ownership.md` §2.1's *handle resolution* clause applies. Verified against the C
in each case:

| Function | tsv `transitive_globals` | Added here | Why |
| --- | --- | --- | --- |
| `window_manager_is_window_eligible` | `-` | WindowManager | `window->is_root`, `window_is_real(window)`, `window_check_rule_flag(window, ..)` all resolve the `WindowId` |
| `window_manager_rule_matches_window` | `-` | WindowManager | `window->application->name` (`:94`) resolves a `ProcessId` through `WindowManager::application`. `patterns/state-and-ownership.md` §3.2 names this exact site as the reason `Application::name` is `Arc<str>` |
| `window_manager_move_window`, `window_manager_resize_window` | `-` | WindowManager | `window->ref` |
| `window_manager_set_window_frame` | `-` | WindowManager | `window->application->ref`, `window->ref` |
| `window_manager_adjust_layer` | `-` | WindowManager | `window->layer`, `window->id` |
| `window_manager_minimize_window`, `window_manager_deminimize_window`, `window_manager_close_window`, `window_manager_toggle_window_shadow` | `-` | WindowManager | `window->ref` / `window->id` |
| `window_manager_toggle_window_expose` | `g_event_bytes` | WindowManager | `window->application->psn`, `window->id`, `window->ref` |
| `window_manager_find_closest_managed_window_in_direction` | no `g_space_manager` | SpaceManager | `view_find_window_node(view, ..)` and `view_find_window_node_in_direction(view, ..)` resolve the `SpaceId` that `window_manager_find_managed_window` now returns |
| the six `window_manager_find_*_for_managed_window` (`:1176`-`:1252`) | `-` | WindowManager, SpaceManager | same — they walk the resolved `View`'s node arena |
| `window_manager_toggle_window_zoom_parent`, `window_manager_toggle_window_zoom_fullscreen` | no `g_space_manager` | SpaceManager | `view->layout`, `view->root`, `view->sid`, `view_set_flag(view, ..)` |
| `window_manager_query_windows_for_spaces` / `_display` / `_displays` | no `g_space_manager` | SpaceManager | via `window_serialize`, whose `view_find_window_node` (`window.c:445`) resolves a view — `patterns/state-and-ownership.md` §2.3 example 2 |
| `window_manager_existing_application_window_list` | `g_window_manager` present | (none added) | `application->connection` resolves the `ProcessId`; already covered |
| `window_manager_add_managed_window` | `-` | SpaceManager | the `struct view *` parameter becomes `&mut SpaceManager` + `SpaceId` by §2.2 step 2, and `view->layout` is read |
| `window_manager_validate_windows_on_space`, `window_manager_check_for_windows_on_space` | `g_space_manager` present | (none added) | same step-2 rewrite of the `struct view *` parameter |

Conversely, two rows where the tsv lists a global that contributes **no** parameter:

| Function | tsv global | Why it contributes nothing |
| --- | --- | --- |
| `window_manager_set_focus_follows_mouse` | `g_mouse_state` | It passes `&g_mouse_state` to `mouse_handler_end` / `mouse_handler_begin`, which touch only `handle` and `runloop_source` — the `MOUSE_TAP_STATE` static half of `DECISIONS.md` 23. No `MouseDragState` |
| `window_manager_begin` | `g_process_manager` | `window_manager.c:2740` walks `g_process_manager.process`, which is the `PROCESS_TABLE` static (`DECISIONS.md` 22). `ProcessManager` enters this row only through the transitive closure below |

`g_process_manager` splits: the three sites that read `finder_psn` (`:1927`, `:2104`, `:2475`, in
`window_manager_warp_window`, `window_manager_send_window_to_space` and
`window_manager_toggle_scratchpad_window`) give `&mut ProcessManager`; the table walk gives nothing.

`g_mission_control_mode` appears in seven rows' `transitive_globals` (via
`mission_control_is_active`, `mission_control.c:108`) and **does** contribute a parameter,
`mission_control_mode: &mut MissionControlMode`, appended last. `patterns/state-and-ownership.md`
§2.2 step 3 enumerates five managers because §2.1 defines `Managers(f)` over managers;
`TRANSLATION_PLAN.md` §3.2 — the spec for this wave — names `g_mission_control_mode` among the
eight event-loop-owned globals to scan for and fixes the append order as the
`EventLoopOwnedState` field order (`patterns/state-and-ownership.md` §1.1), in which
`mission_control_mode` (`yabai.c:37`) is the last appendable field. `DECISIONS.md` 13 forbids
`mission_control_is_active` from reaching the field through a global, so `state-access/event_loop.md`
gives it the parameter and the closure carries it here. The rows are
`window_manager_apply_rule_effects_to_window`, `_apply_rules_to_window`,
`_create_and_add_window`, `_add_application_windows`, `_add_existing_application_windows`,
`_scratchpad_recover_windows` and `window_manager_begin`.

`g_mouse_state`'s drag half reaches sixteen further rows that the C never gave it, through
`view_free_node` (`patterns/state-and-ownership.md` §5.4), which clears
`MouseDragState::feedback_node` when a node slot is freed — something the C never does. Every
caller of `view_remove_window_node` / `window_node_destroy` / `view_clear` / `view_destroy`
therefore takes `mouse_drag_state: &mut MouseDragState`; see `state-access/view.md` N12 and
`state-access/signal-rule-mouse.md` §8.7 for the same chain derived from the other two ends.

## 5. Cross-module calls this module makes

Argument shapes this module expects to pass. Managers are omitted where the callee's own row will
fix them; what matters here is the handle/owned shape of the non-manager arguments.

| Callee | Defined in | Shape expected from here |
| --- | --- | --- |
| `window_create` | `window.c` | `(application: ProcessId, window_ref: AXUIElementRef, window_id: WindowId, window_manager) -> Window`; non-nullable (the C `malloc`s and always returns), and the value is moved into `window_manager_add_window` |
| `window_destroy` | `window.c` | `(mut window: Window)` — the value the table would have owned |
| `window_serialize` | `window.c` | `(response, window_id: WindowId, flags: u64, display_manager, window_manager, space_manager, mouse_drag_state)` — §2.3 example 2, plus the `DisplayManager` that example omits: `window.c:533` calls `display_manager_display_id_arrangement`, and `state-access.tsv` lists `g_display_manager` in the row. Note `window_manager.c:1531` and `:1547` pass **`stdout`**, not the client socket; `Response` as specified in `patterns/message-and-serialisation.md` §7.1 wraps only a `UnixStream` |
| `window_nonax_serialize` | `window.c` | `(response, window_id: WindowId, flags: u64, display_manager)` |
| `window_title_ts`, `window_role_ts`, `window_subrole_ts` | `window.c` | `(window: &Window) -> String` — the record, not the handle (`state-access/window-application-process.md` §1); never `None`, because `window.c:726`, `:1004` and `:1025` return `ts_string_copy("")` on a missing value, so they are passed on as `&str` |
| `window_observe`, `window_unobserve` | `window.c` | `(window: &mut Window, window_manager) -> bool` / `()` — `THREADS.md` §5.4 fixes the record form; the manager is appended after it, per §2.2 step 3 |
| `window_space`, `window_display_id` | `window.c` | `(window_id: WindowId) -> SpaceId` / `DisplayId` |
| `window_ax_frame` | `window.c` | `(window: &Window) -> CGRect` — no manager; `window_create` calls it before the record is in the table |
| `window_is_real`, `window_is_unknown`, `window_is_standard`, `window_is_fullscreen`, `window_is_undersized`, `window_can_move`, `window_can_resize`, `window_can_minimize`, `window_level_is_standard` | `window.c` | `(window: &Window) -> bool` — the record, no manager. `window_is_root` is the one of the family that takes `(window: &Window, window_manager)` |
| `window_is_sticky` | `window.c` | `(window_id: WindowId) -> bool` — takes the raw id in C, no manager |
| `window_set_flag` / `window_clear_flag` / `window_check_flag` and the `rule_flag` trio | `window.h` | `(window: &mut Window, flag: WindowFlag)` on the resolved record, not on the handle |
| `application_create`, `application_observe`, `application_unobserve`, `application_destroy` | `application.c` | `application_create(process: &Arc<Process>) -> Application`, moved into `window_manager_add_application`; `application_observe` / `_unobserve` take `(application: &mut Application)` and `application_destroy` takes `(application: Application)` — all on the record, none on the handle, and none takes a manager |
| `application_window_list` | `application.c` | `(application: &Application) -> Option<CFRetained<CFArray>>` |
| `application_focused_window` | `application.c` | `(application: &Application) -> WindowId` |
| `space_manager_find_view` | `space_manager.c` | `(space_manager, space_id: SpaceId, display_manager, window_manager) -> SpaceId` — it inserts on demand, so it may never be called while a `&mut View` from `SpaceManager::view` is live (`patterns/state-and-ownership.md` §4.2) |
| `space_manager_active_space` | `space_manager.c` | `(window_manager) -> SpaceId` |
| `space_manager_untile_window` | `space_manager.c` | `(space_manager, space_id: SpaceId, window_id: WindowId, ..)` |
| `space_manager_tile_window_on_space` | `space_manager.c` | `(space_manager, window_id: WindowId, space_id: SpaceId, ..) -> SpaceId` — the returned view handle feeds `window_manager_add_managed_window` |
| `space_manager_tile_window_on_space_with_insertion_point` | `space_manager.c` | `(space_manager, window_id: WindowId, space_id: SpaceId, insertion_window_id: WindowId, ..)` |
| `space_manager_move_window_to_space` | `space_manager.c` | `(space_id: SpaceId, window_id: WindowId, window_manager)` |
| `space_manager_focus_space` | `space_manager.c` | `(space_id: SpaceId, window_manager, ..)` |
| `space_manager_mark_view_invalid`, `space_manager_refresh_view` | `space_manager.c` | `(space_manager, space_id: SpaceId, ..)` |
| `space_manager_refresh_application_windows` | `space_manager.c` | `(space_manager, window_manager, process_manager, display_manager, mouse_drag_state)` |
| `space_window_list`, `space_window_list_for_connection` | `space.c` | `(space_id: SpaceId, ..) -> Option<Vec<WindowId>>` / `(space_list: &[SpaceId], connection_id: i32, .., window_manager) -> Option<Vec<WindowId>>`; the out-parameter count is gone, `None` where the C returns NULL |
| `space_is_visible`, `space_is_user`, `space_is_fullscreen` | `space.c` | `(space_id: SpaceId) -> bool` |
| `view_find_window_node`, `view_find_window_node_in_direction`, `view_add_window_node`, `view_add_window_node_with_insertion_point`, `view_remove_window_node`, `view_stack_window_node` | `view.c` | `(space_manager, space_id: SpaceId, .. ) -> Option<NodeId>` — a node crossing a `View` boundary is the pair `(SpaceId, NodeId)` |
| `view_find_window_list` | `view.c` | `(space_manager, space_id: SpaceId) -> Vec<WindowId>` — out-parameter count gone |
| `view_update`, `view_flush`, `view_is_dirty`, `view_set_flag`, `view_clear_flag`, `view_check_flag` | `view.c` | `(space_manager, space_id: SpaceId, ..)` |
| `window_node_flush`, `window_node_update`, `window_node_capture_windows`, `window_node_fence`, `window_node_find_{first,last,next,prev}_leaf`, `window_node_is_leaf`, `window_node_is_left_child`, `window_node_contains_window`, `window_node_swap_window_list`, `window_node_get_{split,gap,ratio}` | `view.c` | `(space_manager, space_id: SpaceId, node_id: NodeId, ..)`; `window_node_capture_windows` returns / extends a `Vec<WindowCapture>` |
| `insert_feedback_show`, `insert_feedback_destroy` | `view.c` | `(space_id: SpaceId, node_id: NodeId, window_manager, space_manager)` — §2.3 example 1 verbatim |
| `area_make_pair` | `view.c` | `(split, gap, ratio, area: &Area) -> (Area, Area)` |
| `display_bounds_constrained` | `display.c` | `(display_id: DisplayId, ignore_docks_and_menubar: bool, display_manager) -> CGRect` |
| `display_space_id`, `display_space_list` | `display.c` | `(display_id: DisplayId) -> SpaceId` / `-> Vec<SpaceId>` |
| `display_manager_active_display_list` | `display_manager.c` | `() -> Vec<DisplayId>` — out-parameter count gone |
| `display_manager_display_is_animating` | `display_manager.c` | `(display_id: DisplayId) -> bool` |
| `mouse_handler_begin`, `mouse_handler_end` | `mouse_handler.c` | `(mask: u32) -> bool` / `()` — the `struct mouse_state *` parameter is gone; both reach only `MOUSE_TAP_STATE` |
| `rule_serialize` | `rule.c` | `(response, rule: &Rule, index: i32, display_manager)` |
| `rule_combine_effects` | `rule.c` | `(source: &RuleEffects, result: &mut RuleEffects)` |
| `rule_destroy` | `rule.c` | **gone** — `Rule`'s `Drop` (`DECISIONS.md` 26); `buf_del` becomes `Vec::swap_remove` |
| `event_loop_post` | `event_loop.c` | `event_loop_post(Event::WindowFocused(window_id))` — the `struct event_loop *` parameter is gone (`EVENT_SENDER` static) and the payload is the variant |
| `workspace_application_is_observable` | `workspace.m` | `(process: &Arc<Process>) -> bool` |
| `workspace_application_observe_activation_policy` | `workspace.m` | `(process: &Arc<Process>)` — `g_workspace_context` is the `WORKSPACE_CONTEXT` static |
| `scripting_addition_set_{opacity,layer,shadow,sticky}`, `_order_window`, `_order_window_in`, `_scale_window`, `_swap_window_proxy_{in,out}` | `sa.m` | `(window_id: WindowId, ..) -> bool`; the two proxy swappers take `&[WindowAnimation]`, the count collapsing into the slice |
| `table_find` / `table_add` / `table_remove` | `misc/hashtable.h` | methods on `Table<K, V>`, keyed by the handle newtype |
| `string_equals` | `misc/helpers.h` | plain `left == right` at `window_manager.c:180-205`, whose four values are all `_ts` results and never NULL (judgement 11); `Option<&str>` on both sides at `:2434`, where `Window::title` really is nullable (`patterns/memory-text-and-os-objects.md` §2.2) |
| `regex_match` | `misc/helpers.h` | `(is_valid: bool, regex: &PosixRegex, text: &CStr) -> RegexMatch` — `patterns/message-and-serialisation.md:1731` and `sweeps/c-idioms-and-semantics.md:840` both take a C string, not a `&str`; the misc unit owns the row and the two sketches there disagree on the `valid` flag, which is outside these nine files |
| `psn_equals` | `misc/helpers.h` | `(first: &ProcessSerialNumber, second: &ProcessSerialNumber) -> bool` |
| `cgimage_restore_alpha`, `sls_window_disable_shadow`, `mach_send`, `ax_window_id`, `clampf_range`, `rgba_color_from_hex`, `string_copy` | `misc/helpers.h` | unchanged shapes; `string_copy(effects.scratchpad)` yields the `String` moved into `window_manager_set_scratchpad_for_window` |

## 6. Judgement calls recorded in this document

1. **`struct window *` parameters become `WindowId`, not `&Window`.** §2.2 step 1 and §2.3 example 2
   are binding. `patterns/state-and-ownership.md` §4.2's R2 sketch passes a resolved `&Window` to
   `window_manager_is_window_eligible`; that is prose about a body, and the signature rule wins.
2. **`window_manager_begin` takes five managers, not two.** §2.3 example 6 states
   `Managers = { SpaceManager, WindowManager }`, but it only evaluates `Direct` — it never closes
   over `window_manager_add_existing_application_windows` (`:2745`), which needs `ProcessManager`,
   `DisplayManager` and `MouseDragState`. §2.1 requires the least fixed point and says a
   caller/callee disagreement means the fixed point was computed wrong. Applied the fixed point.
3. **`response: &mut Response`, not `&mut dyn std::io::Write`.** `GLOSSARY.md` §10.1 and §10.4 and
   `DECISIONS.md` 28 name the `Response` type; §2.2's `dyn Write` phrasing predates it.
4. **`a` / `b` become `a_window` / `b_window`.** Settled by the cross-module pass, against this
   file's first reading. `GLOSSARY.md` §10.3 names `window_manager.c:1799`, `:1832` and `:1950`
   by line and fixes `a_window` / `b_window` for them; §10.2's `a_window_id` / `b_window_id` row
   is about a different C spelling at a different site (`a_wid` / `b_wid`, `sa.m:576`). The
   more specific rule wins, and `state-access/message.md`'s call shapes were corrected to match.
5. **`struct rule *` and `struct rule_effects *` stay borrows (`&Rule`, `&RuleEffects`).** §3
   enumerates the five pointer kinds that become handles and `struct rule *` is not one.
   `window_manager_rule_matches_window` therefore holds `&Rule` *and* `&mut WindowManager`; the
   rules live in `WindowManager::rules`, so the two loops at `:174` and `:198` must
   `std::mem::take` the `Vec<Rule>` for the duration and put it back — `Rule` is deliberately not
   `Clone` (`patterns/message-and-serialisation.md` §10.5). If wave 1 prefers, the alternative is
   `rule_index: usize`; it must be decided once for `rule_apply` (`rule.c:158`) at the same time.
6. **`window_manager_build_window_proxy_thread_proc` takes one parameter, not two.**
   `THREADS.md` §2.6 sketches
   `window_manager_build_window_proxy_thread_proc(window_animation, &animation_context)`, but the C
   body (`:507-533`) reads only `animation->*`, including `animation->cid` where the context would
   have been consulted. Kept the C's single parameter, as `&mut WindowAnimation` — the builder
   writes `proxy.level`, `proxy.sub_level`, `proxy.frame` and `proxy.image`, which `.iter()` over an
   `Arc<AnimationContext>` cannot provide. The C mutates the batch before the context is handed to
   the display link (`:701`), so `iter_mut()` under `thread::scope` reproduces it exactly.
7. **`window_manager_animate_window_list_thread_proc` takes no manager**, per `DECISIONS.md` 20 and
   `THREADS.md` §2.5, yet the C body locks and mutates `g_window_manager.window_animations_table`
   (`:577-589`). `GLOSSARY.md` §3.1 already makes that field
   `Arc<Mutex<Table<WindowId, (Arc<AnimationContext>, usize)>>>`; a clone of that `Arc` must reach
   the callback through the context, and `GLOSSARY.md` §3.15 has no field for it. Flagged for
   `W1-view` / `W1-window_manager` rather than invented here.
8. **`window_manager_find_window` returns `Option<WindowId>`**, and `window_manager_find_application`
   `Option<ProcessId>`, per the "returned record pointer becomes `Option` of its handle" rule. Both
   degenerate into existence checks, which is what most C call sites use them for; bodies that need
   the record resolve it through `WindowManager::window` / `::application` at the point of use (R1).
   `window_manager_find_managed_window -> Option<SpaceId>` is stated outright in §3.2, which is the
   pattern these two follow.
9. **`window_manager_add_window` and `window_manager_add_application` take the record by value.**
   The tables own `Window` and `Application` (§3.2), so the C's "insert this pointer" is a move.
10. **Non-pointer scalar parameters keep their C type**, mapped straight across: `int` → `i32`,
    `unsigned` → `u32`, `float` → `f32`, `bool` → `bool`. In particular `direction` in
    `window_manager_resize_window_relative` / `_internal` stays `i32` even though its values are
    `HANDLE_*` bits and `MouseDragState::direction` is a `u8` "tested with `ResizeHandle`"
    (`GLOSSARY.md` §3.23); the `mouse_handler.c:245` / `:254` call sites cast. `type` becomes
    `type_of_change` and `r`/`c` become `rows`/`columns` per `GLOSSARY.md` §11 and §10.4.
11. **`window_title` / `window_role` / `window_subrole` are `&str`, not `Option<&str>`.**
    Settled by the cross-module pass, against this file's first reading.
    `window_{title,role,subrole}_ts` cannot return NULL: `window.c:726`, `:1004` and `:1025`
    return `ts_string_copy("")` when the `CFStringRef` is absent, and
    `state-access/window-application-process.md` types all three `-> String`. Every value that
    reaches these three parameters is one of those `_ts` results — `rule.c:121`, `:125`, `:163`
    and `window_manager.c:178`, `:202`, `:1487`, `:1491`, which is all of them — so the parameter
    is never `None`. `patterns/memory-text-and-os-objects.md` §2.2 lists `window_manager.c:180-205`
    among the "genuinely nullable" `string_equals` sites; the C type is nullable there, the values
    are not, so those sites take that section's second form, plain `left == right`.
12. **`hash_wm` is renamed** to `hash_window_manager_key(key: &u32) -> u64` by
    `patterns/state-and-ownership.md` §1.3 — the one function in this pair that does not keep its C
    name. Its `&u32` key does not match `Table<ProcessId, Application>`'s `fn(&K) -> u64` field
    (`GLOSSARY.md` §3.26); `W1-1` must either make it generic or mint a second one for `ProcessId`.
13. **Visibility follows the tsv's `called_from_other_files` column mechanically.** Six functions
    are declared in `window_manager.h` yet have no caller outside `window_manager.c`, so they are
    private here: `window_manager_resize_window` (`:425`),
    `window_manager_animate_window_list_async` (`:603`), `window_manager_set_window_frame` (`:729`),
    `window_manager_add_window` (`:1404`), `window_manager_focused_application` (`:1339`) and
    `window_manager_toggle_scratchpad_window` (`:2448`). Each header declaration becomes one
    `DEVIATIONS.md` line if wave 1 keeps them private.
14. **`hash_wm`'s thread context is "any"** — it is a function pointer stored by
    `window_manager_init` on the main thread at start-up and invoked on every table operation, which
    happens on EVENTLOOP and (through `window_animations_table`) on the display-link thread.
