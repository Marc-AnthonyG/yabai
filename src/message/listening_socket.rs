use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use crate::command::DaemonCommand;
use crate::event::queue::{Event, post_event_to_event_loop};
use crate::protocol::reply::{DaemonReply, write_reply};
use crate::protocol::request::{
    command_of_a_request_from_a_client_of_the_same_version, read_request_after_its_length,
};

const LONGEST_A_CLIENT_MAY_TAKE_TO_SEND_ITS_REQUEST: Duration = Duration::from_secs(1);

pub(crate) struct MessageSocketListener {
    pub(crate) listener: UnixListener,
}

pub(crate) static MESSAGE_SOCKET_LISTENER: OnceLock<MessageSocketListener> = OnceLock::new();

pub(crate) fn accept_message_connections_and_post_them_to_the_event_loop() {
    let Some(message_loop) = MESSAGE_SOCKET_LISTENER.get() else {
        return;
    };
    for stream in message_loop.listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };

        match read_the_command_a_client_sends(&stream) {
            Ok(command) => post_event_to_event_loop(Event::DaemonCommand {
                command,
                reply_to: stream,
            }),
            Err(failure) => {
                let _ = write_reply(&stream, &DaemonReply::failing_with(failure));
            }
        }
    }
}

fn read_the_command_a_client_sends(stream: &UnixStream) -> Result<DaemonCommand, String> {
    let _ = stream.set_read_timeout(Some(LONGEST_A_CLIENT_MAY_TAKE_TO_SEND_ITS_REQUEST));
    let request = read_request_after_its_length(stream)?;
    command_of_a_request_from_a_client_of_the_same_version(request)
}

pub(crate) fn start_listening_on_message_socket(socket_path: &Path) -> bool {
    let mut socket_address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    socket_address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let socket_path_bytes = socket_path.as_os_str().as_bytes();
    let socket_path_length = socket_path_bytes
        .len()
        .min(socket_address.sun_path.len() - 1);
    for index in 0..socket_path_length {
        socket_address.sun_path[index] = socket_path_bytes[index] as libc::c_char;
    }
    let _ = std::fs::remove_file(socket_path);

    let socket_file_descriptor = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if socket_file_descriptor == -1 {
        return false;
    }
    let socket = unsafe { OwnedFd::from_raw_fd(socket_file_descriptor) };

    if unsafe {
        libc::bind(
            socket_file_descriptor,
            &socket_address as *const libc::sockaddr_un as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        )
    } == -1
    {
        return false;
    }

    if std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600)).is_err() {
        return false;
    }

    if unsafe { libc::listen(socket_file_descriptor, libc::SOMAXCONN) } == -1 {
        return false;
    }

    unsafe {
        libc::fcntl(
            socket_file_descriptor,
            libc::F_SETFD,
            libc::FD_CLOEXEC | libc::fcntl(socket_file_descriptor, libc::F_GETFD),
        )
    };

    let listener = UnixListener::from(socket);
    let _ = MESSAGE_SOCKET_LISTENER.set(MessageSocketListener { listener });
    let _ = std::thread::Builder::new()
        .spawn(accept_message_connections_and_post_them_to_the_event_loop);

    true
}
