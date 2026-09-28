use core::ffi::c_int;
use std::io::Read;
use std::os::fd::IntoRawFd;
use std::os::unix::net::UnixStream;

use crate::display::manager::DisplayManager;
use crate::message::dispatch::dispatch_message_to_its_domain;
use crate::mouse::drag::MouseDragState;
use crate::process::manager::ProcessManager;
use crate::signal::definition::{SIGNAL_TYPE_COUNT, Signal};
use crate::space::manager::SpaceManager;
use crate::state::mission_control_mode::MissionControlMode;
use crate::support::log::print_message_arguments_when_verbose;
use crate::support::response::Response;
use crate::support::sockets::shut_down_and_close_socket;
use crate::window::manager::WindowManager;

pub(crate) fn handle_daemon_message_event(
    stream: UnixStream,
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    process_manager: &mut ProcessManager,
    display_manager: &mut DisplayManager,
    window_manager: &mut WindowManager,
    space_manager: &mut SpaceManager,
    mouse_drag_state: &mut MouseDragState,
    mission_control_mode: &mut MissionControlMode,
) {
    let mut stream = stream;
    let mut bytes_read: i32 = 0;
    let mut bytes_to_read_bytes = [0u8; size_of::<c_int>()];

    let prefix_read = stream.read(&mut bytes_to_read_bytes);
    if prefix_read.is_ok_and(|count| count == size_of::<c_int>()) {
        let bytes_to_read = c_int::from_ne_bytes(bytes_to_read_bytes);

        if bytes_to_read > 0 {
            let mut message = vec![0u8; bytes_to_read as usize + 2];

            loop {
                let current_read =
                    match stream.read(&mut message[bytes_read as usize..bytes_to_read as usize]) {
                        Ok(count) => count as i32,
                        Err(_) => -1,
                    };
                if current_read <= 0 {
                    break;
                }

                bytes_read += current_read;
                if !(bytes_read < bytes_to_read) {
                    break;
                }
            }

            if bytes_read == bytes_to_read {
                let mut response = Response::to_client(stream);
                print_message_arguments_when_verbose(
                    "handle_daemon_message_event",
                    &String::from_utf8_lossy(&message),
                );
                dispatch_message_to_its_domain(
                    &mut response,
                    &mut message,
                    signal_event,
                    process_manager,
                    display_manager,
                    window_manager,
                    space_manager,
                    mouse_drag_state,
                    mission_control_mode,
                );

                drop(response);

                return;
            }
        }
    }

    shut_down_and_close_socket(stream.into_raw_fd());
}
