use std::sync::mpsc::Receiver;

use objc2::rc::autoreleasepool;

use crate::event::handlers::application::{
    handle_application_front_switched_event, handle_application_hidden_event,
    handle_application_launched_event, handle_application_terminated_event,
    handle_application_visible_event,
};
use crate::event::handlers::daemon_command::handle_daemon_command_event;
use crate::event::handlers::display::{
    handle_display_added_event, handle_display_changed_event, handle_display_moved_event,
    handle_display_removed_event, handle_display_resized_event,
};
use crate::event::handlers::insert_feedback::handle_insert_feedback_fade_in_step_event;
use crate::event::handlers::menu::{handle_menu_closed_event, handle_menu_opened_event};
use crate::event::handlers::mission_control::{
    handle_mission_control_check_for_exit_event, handle_mission_control_enter_event,
    handle_mission_control_exit_event, handle_mission_control_show_all_windows_event,
    handle_mission_control_show_desktop_event, handle_mission_control_show_front_windows_event,
};
use crate::event::handlers::mouse::{
    handle_focus_follows_mouse_under_the_still_cursor_event, handle_mouse_down_event,
    handle_mouse_dragged_event, handle_mouse_moved_event, handle_mouse_up_event,
};
use crate::event::handlers::space::{
    handle_skylight_space_created_event, handle_skylight_space_destroyed_event,
    handle_space_changed_event,
};
use crate::event::handlers::system::{
    handle_dock_did_change_preferences_event, handle_dock_did_restart_event,
    handle_menu_bar_hidden_changed_event, handle_system_accent_color_changed_event,
    handle_system_woke_event,
};
use crate::event::handlers::window::{
    handle_skylight_window_destroyed_event, handle_skylight_window_ordered_event,
    handle_window_created_event, handle_window_deminimized_event, handle_window_destroyed_event,
    handle_window_focused_event, handle_window_minimized_event, handle_window_moved_event,
    handle_window_resized_event, handle_window_title_changed_event,
};
use crate::event::queue::Event;
use crate::signal::exec::run_subscriber_commands_of_pending_signals_without_waiting_for_them;
use crate::state::event_loop_owned::EventLoopOwnedState;

pub(crate) fn run_event_loop_flushing_signals_after_each_event(
    event_receiver: Receiver<Event>,
    mut event_loop_owned_state: EventLoopOwnedState,
) {
    let EventLoopOwnedState {
        signal_event,
        process_manager,
        display_manager,
        window_manager,
        space_manager,
        signal_storage,
        mouse_drag_state,
        mission_control_mode,
        focus_follows_mouse_suspended_value,
        is_menu_open,
    } = &mut event_loop_owned_state;

    while let Ok(first_event_of_batch) = event_receiver.recv() {
        autoreleasepool(|_| {
            let mut next = first_event_of_batch;

            loop {
                match next {
                    Event::ApplicationLaunched(process) => handle_application_launched_event(
                        process,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::ApplicationTerminated(process) => handle_application_terminated_event(
                        process,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::ApplicationFrontSwitched(process) => {
                        handle_application_front_switched_event(
                            process,
                            signal_event,
                            process_manager,
                            display_manager,
                            window_manager,
                            space_manager,
                            signal_storage,
                            mouse_drag_state,
                            mission_control_mode,
                        )
                    }
                    Event::ApplicationVisible(process_id) => handle_application_visible_event(
                        process_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::ApplicationHidden(process_id) => handle_application_hidden_event(
                        process_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowCreated(element_ref) => handle_window_created_event(
                        element_ref,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::WindowDestroyed(window_id) => handle_window_destroyed_event(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowFocused(window_id) => handle_window_focused_event(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowMoved(window_id) => handle_window_moved_event(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowResized(window_id) => handle_window_resized_event(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowMinimized(window_id) => handle_window_minimized_event(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowDeminimized(window_id) => handle_window_deminimized_event(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::WindowTitleChanged(window_id) => handle_window_title_changed_event(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::SkylightWindowOrdered(window_id) => {
                        handle_skylight_window_ordered_event(
                            window_id,
                            window_manager,
                            space_manager,
                        )
                    }
                    Event::SkylightWindowDestroyed(window_id) => {
                        handle_skylight_window_destroyed_event(
                            window_id,
                            signal_event,
                            process_manager,
                            display_manager,
                            window_manager,
                            space_manager,
                            signal_storage,
                            mouse_drag_state,
                        )
                    }
                    Event::SkylightSpaceCreated(space_id) => handle_skylight_space_created_event(
                        space_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::SkylightSpaceDestroyed(space_id) => {
                        handle_skylight_space_destroyed_event(
                            space_id,
                            signal_event,
                            process_manager,
                            display_manager,
                            window_manager,
                            space_manager,
                            signal_storage,
                            mouse_drag_state,
                        )
                    }
                    Event::SpaceChanged => handle_space_changed_event(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::DisplayAdded(display_id) => handle_display_added_event(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::DisplayRemoved(display_id) => handle_display_removed_event(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::DisplayMoved(display_id) => handle_display_moved_event(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DisplayResized(display_id) => handle_display_resized_event(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DisplayChanged => handle_display_changed_event(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MouseDown {
                        event,
                        event_modifier,
                    } => handle_mouse_down_event(
                        event,
                        event_modifier,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MouseUp { event } => handle_mouse_up_event(
                        event,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MouseDragged { event } => handle_mouse_dragged_event(
                        event,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MouseMoved {
                        event,
                        event_modifier,
                    } => handle_mouse_moved_event(
                        event,
                        event_modifier,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MissionControlShowAllWindows => {
                        handle_mission_control_show_all_windows_event(
                            signal_event,
                            process_manager,
                            display_manager,
                            window_manager,
                            space_manager,
                            signal_storage,
                            mission_control_mode,
                        )
                    }
                    Event::MissionControlShowFrontWindows => {
                        handle_mission_control_show_front_windows_event(
                            signal_event,
                            process_manager,
                            display_manager,
                            window_manager,
                            space_manager,
                            signal_storage,
                            mission_control_mode,
                        )
                    }
                    Event::MissionControlShowDesktop => handle_mission_control_show_desktop_event(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mission_control_mode,
                    ),
                    Event::MissionControlEnter => handle_mission_control_enter_event(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mission_control_mode,
                    ),
                    Event::MissionControlCheckForExit => {
                        handle_mission_control_check_for_exit_event(mission_control_mode)
                    }
                    Event::MissionControlExit => handle_mission_control_exit_event(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::DockDidRestart => handle_dock_did_restart_event(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::MenuOpened(window_id) => handle_menu_opened_event(
                        window_id,
                        window_manager,
                        focus_follows_mouse_suspended_value,
                        is_menu_open,
                    ),
                    Event::MenuClosed => handle_menu_closed_event(
                        window_manager,
                        focus_follows_mouse_suspended_value,
                        is_menu_open,
                    ),
                    Event::MenuBarHiddenChanged => handle_menu_bar_hidden_changed_event(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DockDidChangePreferences => handle_dock_did_change_preferences_event(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::SystemWoke => handle_system_woke_event(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::SystemAccentColorChanged(accent_color) => {
                        handle_system_accent_color_changed_event(accent_color, window_manager)
                    }
                    Event::InsertFeedbackFadeInStep => {
                        handle_insert_feedback_fade_in_step_event(space_manager)
                    }
                    Event::FocusFollowsMouseUnderTheStillCursor {
                        new_window_that_keeps_its_focus,
                    } => handle_focus_follows_mouse_under_the_still_cursor_event(
                        new_window_that_keeps_its_focus,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::DaemonCommand { command, reply_to } => handle_daemon_command_event(
                        command,
                        reply_to,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                }

                run_subscriber_commands_of_pending_signals_without_waiting_for_them(
                    signal_event,
                    signal_storage,
                );

                match event_receiver.try_recv() {
                    Ok(next_event) => next = next_event,
                    Err(_) => break,
                }
            }
        });
    }
}
