use std::sync::mpsc::Receiver;

use objc2::rc::autoreleasepool;

use crate::event::handlers::application::{
    event_handler_application_front_switched, event_handler_application_hidden,
    event_handler_application_launched, event_handler_application_terminated,
    event_handler_application_visible,
};
use crate::event::handlers::daemon_message::event_handler_daemon_message;
use crate::event::handlers::display::{
    event_handler_display_added, event_handler_display_changed, event_handler_display_moved,
    event_handler_display_removed, event_handler_display_resized,
};
use crate::event::handlers::menu::{event_handler_menu_closed, event_handler_menu_opened};
use crate::event::handlers::mission_control::{
    event_handler_mission_control_check_for_exit, event_handler_mission_control_enter,
    event_handler_mission_control_exit, event_handler_mission_control_show_all_windows,
    event_handler_mission_control_show_desktop, event_handler_mission_control_show_front_windows,
};
use crate::event::handlers::mouse::{
    event_handler_mouse_down, event_handler_mouse_dragged, event_handler_mouse_moved,
    event_handler_mouse_up,
};
use crate::event::handlers::space::{
    event_handler_sls_space_created, event_handler_sls_space_destroyed, event_handler_space_changed,
};
use crate::event::handlers::system::{
    event_handler_dock_did_change_pref, event_handler_dock_did_restart,
    event_handler_menu_bar_hidden_changed, event_handler_system_woke,
};
use crate::event::handlers::window::{
    event_handler_sls_window_destroyed, event_handler_sls_window_ordered,
    event_handler_window_created, event_handler_window_deminimized, event_handler_window_destroyed,
    event_handler_window_focused, event_handler_window_minimized, event_handler_window_moved,
    event_handler_window_resized, event_handler_window_title_changed,
};
use crate::event::queue::Event;
use crate::signal::exec::event_signal_flush;
use crate::state::event_loop_owned::EventLoopOwnedState;

pub(crate) fn event_loop_run(
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
                    Event::ApplicationLaunched(process) => event_handler_application_launched(
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
                    Event::ApplicationTerminated(process) => event_handler_application_terminated(
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
                        event_handler_application_front_switched(
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
                    Event::ApplicationVisible(process_id) => event_handler_application_visible(
                        process_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::ApplicationHidden(process_id) => event_handler_application_hidden(
                        process_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowCreated(element_ref) => event_handler_window_created(
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
                    Event::WindowDestroyed(window_id) => event_handler_window_destroyed(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowFocused(window_id) => event_handler_window_focused(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowMoved(window_id) => event_handler_window_moved(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowResized(window_id) => event_handler_window_resized(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowMinimized(window_id) => event_handler_window_minimized(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::WindowDeminimized(window_id) => event_handler_window_deminimized(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::WindowTitleChanged(window_id) => event_handler_window_title_changed(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::SlsWindowOrdered(window_id) => {
                        event_handler_sls_window_ordered(window_id, window_manager, space_manager)
                    }
                    Event::SlsWindowDestroyed(window_id) => event_handler_sls_window_destroyed(
                        window_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::SlsSpaceCreated(space_id) => event_handler_sls_space_created(
                        space_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::SlsSpaceDestroyed(space_id) => event_handler_sls_space_destroyed(
                        space_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::SpaceChanged => event_handler_space_changed(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::DisplayAdded(display_id) => event_handler_display_added(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::DisplayRemoved(display_id) => event_handler_display_removed(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                    ),
                    Event::DisplayMoved(display_id) => event_handler_display_moved(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DisplayResized(display_id) => event_handler_display_resized(
                        display_id,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DisplayChanged => event_handler_display_changed(
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
                    } => event_handler_mouse_down(
                        event,
                        event_modifier,
                        window_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MouseUp { event } => event_handler_mouse_up(
                        event,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MouseDragged { event } => event_handler_mouse_dragged(
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
                    } => event_handler_mouse_moved(
                        event,
                        event_modifier,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::MissionControlShowAllWindows => {
                        event_handler_mission_control_show_all_windows(
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
                        event_handler_mission_control_show_front_windows(
                            signal_event,
                            process_manager,
                            display_manager,
                            window_manager,
                            space_manager,
                            signal_storage,
                            mission_control_mode,
                        )
                    }
                    Event::MissionControlShowDesktop => event_handler_mission_control_show_desktop(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mission_control_mode,
                    ),
                    Event::MissionControlEnter => event_handler_mission_control_enter(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mission_control_mode,
                    ),
                    Event::MissionControlCheckForExit => {
                        event_handler_mission_control_check_for_exit(mission_control_mode)
                    }
                    Event::MissionControlExit => event_handler_mission_control_exit(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                    Event::DockDidRestart => event_handler_dock_did_restart(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::MenuOpened(window_id) => event_handler_menu_opened(
                        window_id,
                        window_manager,
                        focus_follows_mouse_suspended_value,
                        is_menu_open,
                    ),
                    Event::MenuClosed => event_handler_menu_closed(
                        window_manager,
                        focus_follows_mouse_suspended_value,
                        is_menu_open,
                    ),
                    Event::MenuBarHiddenChanged => event_handler_menu_bar_hidden_changed(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DockDidChangePref => event_handler_dock_did_change_pref(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::SystemWoke => event_handler_system_woke(
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        signal_storage,
                    ),
                    Event::DaemonMessage(stream) => event_handler_daemon_message(
                        stream,
                        signal_event,
                        process_manager,
                        display_manager,
                        window_manager,
                        space_manager,
                        mouse_drag_state,
                        mission_control_mode,
                    ),
                }

                event_signal_flush(signal_event, signal_storage);

                match event_receiver.try_recv() {
                    Ok(next_event) => next = next_event,
                    Err(_) => break,
                }
            }
        });
    }
}
