#![allow(deprecated)]

use crate::display::bounds::query_bounds_of_display_left_for_windows;
use crate::display::focus::{focus_window_under_point, move_active_menu_bar_to_display};
use crate::display::identity::query_display_at_point;
use crate::display::manager::DisplayManager;
use crate::event::queue::{Event, post_event_to_event_loop};
use crate::ffi::accessibility::{kAXDrawerRole, kAXSheetRole};
use crate::ffi::core_foundation::{
    CFArrayGetCount, CFEqual, CFIndex, CFNumber, CGPoint, as_cftype, cfarray_borrow_value_at_index,
    cfnumber_read_i32, take_create_rule_result,
};
use crate::ffi::core_graphics::CGRectContainsRect;
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::ffi::skylight::SLSCopyAssociatedWindows;
use crate::mouse::drag::MouseDragState;
use crate::space::managed_space::query_windows_on_space;
use crate::space::manager::SpaceManager;
use crate::state::process_wide::SKYLIGHT_CONNECTION_ID;
use crate::support::geometry::is_point_inside_rectangle_including_its_edges;
use crate::support::handles::WindowId;
use crate::window::focus::{
    focus_and_raise_window_of_process, focus_window_of_process_without_raising_it,
};
use crate::window::manager::{
    FocusFollowsMouseMode, WindowManager, is_window_eligible_for_management, tracked_window_with_id,
};
use crate::window::model::{
    WindowFlag, is_window_flag_set, query_window_level_from_window_server,
    query_window_sub_level_from_window_server, window_role,
};
use crate::window::screen_lookup::query_tracked_window_at_point;

const TIME_FOR_THE_WINDOW_SERVER_TO_SHOW_A_NEW_LAYOUT_IN_NANOSECONDS: i64 = 50_000_000;

pub(crate) fn schedule_focus_follows_mouse_under_the_still_cursor_once_the_layout_settles(
    window_manager: &WindowManager,
) {
    if window_manager.focus_follows_mouse_mode == FocusFollowsMouseMode::Disabled {
        return;
    }
    let animation_in_nanoseconds =
        (window_manager.window_animation_duration.max(0.0) as f64 * 1e9) as i64;
    dispatch_after_on_main_queue(
        animation_in_nanoseconds + TIME_FOR_THE_WINDOW_SERVER_TO_SHOW_A_NEW_LAYOUT_IN_NANOSECONDS,
        || post_event_to_event_loop(Event::FocusFollowsMouseUnderTheStillCursor),
    );
}

pub(crate) fn focus_the_window_at_point_the_way_focus_follows_mouse_does(
    point: CGPoint,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
) {
    let window = query_tracked_window_at_point(window_manager, point);

    if let Some(mut window) = window {
        if window == window_manager.focused_window_id {
            return;
        }
        if !is_window_eligible_for_management(window, window_manager) {
            return;
        }

        if window_manager.focus_follows_mouse_mode == FocusFollowsMouseMode::Autofocus {
            //
            // NOTE(asmvik): Look for a window with role AXSheet or AXDrawer
            // and forward focus to it because we are not allowed to focus the main
            // window in these cases.
            //

            let window_list = unsafe {
                take_create_rule_result(SLSCopyAssociatedWindows(
                    *SKYLIGHT_CONNECTION_ID.get().unwrap(),
                    window.0,
                ))
            };
            if let Some(window_list) = window_list {
                let window_count = CFArrayGetCount(&window_list) as i32;

                for index in 0..window_count {
                    let child_number = unsafe {
                        cfarray_borrow_value_at_index::<CFNumber>(&window_list, index as CFIndex)
                    };
                    let child_window_id =
                        WindowId(child_number.map_or(0, cfnumber_read_i32) as u32);
                    let child = tracked_window_with_id(window_manager, child_window_id);
                    let Some(child) = child else {
                        continue;
                    };

                    let Some(child_record) = window_manager.window.find(&child) else {
                        continue;
                    };
                    let role = window_role(child_record);
                    let Some(role) = role else {
                        continue;
                    };

                    let valid = CFEqual(Some(as_cftype(role)), Some(as_cftype(kAXSheetRole())))
                        || CFEqual(Some(as_cftype(role)), Some(as_cftype(kAXDrawerRole())));

                    if valid {
                        window = child;
                        break;
                    }
                }

                drop(window_list);
            }

            let window_process_serial_number = window_manager
                .window
                .find(&window)
                .and_then(|window| window.application)
                .and_then(|application| window_manager.application.find(&application))
                .map(|application| application.process_serial_number);
            let Some(window_process_serial_number) = window_process_serial_number else {
                return;
            };
            focus_window_of_process_without_raising_it(
                &window_process_serial_number,
                window,
                window_manager,
            );
            mouse_drag_state.ffm_window_id = window;
        } else if window_manager.focus_follows_mouse_mode == FocusFollowsMouseMode::Autoraise {
            //
            // NOTE(asmvik): If any **floating** window would be fully occluded by
            // autoraising the window below the cursor we do not actually perform the
            // focus change, as it is likely that the user is trying to reach for the
            // smaller window that sits on top of the window we would otherwise raise.
            //

            let mut occludes_window = false;

            let window_list =
                query_windows_on_space(space_manager.current_space_id, false, window_manager);

            if let Some(window_list) = window_list {
                let window_count = window_list.len() as i32;
                for index in 0..window_count {
                    let window_id = window_list[index as usize];
                    if window_id == window {
                        break;
                    }

                    let sub_window = tracked_window_with_id(window_manager, window_id);
                    let Some(sub_window) = sub_window else {
                        continue;
                    };
                    let Some(sub_window_record) = window_manager.window.find(&sub_window) else {
                        continue;
                    };

                    if !is_window_flag_set(sub_window_record, WindowFlag::FLOATING) {
                        continue;
                    }
                    if query_window_level_from_window_server(window)
                        != query_window_level_from_window_server(sub_window)
                    {
                        continue;
                    }
                    if query_window_sub_level_from_window_server(window)
                        != query_window_sub_level_from_window_server(sub_window)
                    {
                        continue;
                    }

                    let window_frame = window_manager
                        .window
                        .find(&window)
                        .map(|window| window.frame);
                    if let Some(window_frame) = window_frame
                        && CGRectContainsRect(window_frame, sub_window_record.frame)
                    {
                        occludes_window = true;
                        break;
                    }
                }
            }

            if !occludes_window {
                let Some(window_record) = window_manager.window.find(&window) else {
                    return;
                };
                let window_element_ref = window_record.element_ref;
                let window_process_serial_number = window_record
                    .application
                    .and_then(|application| window_manager.application.find(&application))
                    .map(|application| application.process_serial_number);
                let Some(window_process_serial_number) = window_process_serial_number else {
                    return;
                };
                focus_and_raise_window_of_process(
                    &window_process_serial_number,
                    window,
                    window_element_ref,
                );
                mouse_drag_state.ffm_window_id = window;
            }
        }
    } else {
        let cursor_display_id = query_display_at_point(point);
        if display_manager.current_display_id == cursor_display_id {
            return;
        }

        let bounds =
            query_bounds_of_display_left_for_windows(cursor_display_id, false, display_manager);
        if !is_point_inside_rectangle_including_its_edges(bounds, point) {
            return;
        }

        let window_id = focus_window_under_point(point, window_manager);
        if window_id.0 == 0 {
            move_active_menu_bar_to_display(cursor_display_id);
        }
        mouse_drag_state.ffm_window_id = window_id;
    }
}
