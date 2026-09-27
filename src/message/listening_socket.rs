use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::OnceLock;

use crate::event::queue::{Event, event_loop_post};
use crate::message::token::c_string_at;

pub(crate) struct MessageLoop {
    pub(crate) listener: UnixListener,
}

pub(crate) static MESSAGE_LOOP: OnceLock<MessageLoop> = OnceLock::new();

pub(crate) fn message_loop_run() {
    let Some(message_loop) = MESSAGE_LOOP.get() else {
        return;
    };
    for stream in message_loop.listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };

        event_loop_post(Event::DaemonMessage(stream));
    }
}

pub(crate) fn message_loop_begin(socket_path: &Path) -> bool {
    let mut socket_address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    socket_address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let socket_path_bytes = socket_path.as_os_str().as_bytes();
    let socket_path_length = c_string_at(socket_path_bytes, 0)
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
    let _ = MESSAGE_LOOP.set(MessageLoop { listener });
    let _ = std::thread::Builder::new().spawn(message_loop_run);

    true
}
