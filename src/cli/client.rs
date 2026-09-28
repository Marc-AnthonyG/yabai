use core::ffi::{c_int, c_void};
use std::io::Write;

use crate::error;
use crate::startup::settings_and_lock_file::MESSAGE_SOCKET_PATH_FORMAT;
use crate::support::response::FAILURE_RESPONSE_MARKER;
use crate::support::sockets::{
    connect_socket_to_unix_path, open_unix_stream_socket, shut_down_and_close_socket,
};

pub(crate) fn send_message_to_daemon_and_print_its_response(
    arguments: &[std::ffi::OsString],
) -> i32 {
    let argument_count = arguments.len() as i32;

    if argument_count <= 1 {
        error!("yabai-msg: no arguments given! abort..\n");
    }

    let Some(user) = std::env::var_os("USER") else {
        error!("yabai-msg: 'env USER' not set! abort..\n");
    };
    let user = user.to_string_lossy();

    let mut message_length: c_int = argument_count;
    let mut argument_lengths: Vec<c_int> = vec![0; argument_count as usize];

    for index in 1..argument_count as usize {
        argument_lengths[index] =
            std::os::unix::ffi::OsStrExt::as_bytes(arguments[index].as_os_str()).len() as c_int;
        message_length += argument_lengths[index];
    }

    let mut message: Vec<u8> = Vec::with_capacity(size_of::<c_int>() + message_length as usize);

    message.extend_from_slice(&message_length.to_ne_bytes());
    for index in 1..argument_count as usize {
        message.extend_from_slice(
            &std::os::unix::ffi::OsStrExt::as_bytes(arguments[index].as_os_str())
                [..argument_lengths[index] as usize],
        );
        message.push(b'\0');
    }
    message.push(b'\0');

    let mut socket_file_descriptor: c_int = 0;
    let socket_file = MESSAGE_SOCKET_PATH_FORMAT.replacen("%s", &user, 1);

    if !open_unix_stream_socket(&mut socket_file_descriptor) {
        error!("yabai-msg: failed to open socket..\n");
    }

    if !connect_socket_to_unix_path(socket_file_descriptor, &socket_file) {
        error!("yabai-msg: failed to connect to socket..\n");
    }

    if unsafe {
        libc::send(
            socket_file_descriptor,
            message.as_ptr().cast::<c_void>(),
            size_of::<c_int>() + message_length as usize,
            0,
        )
    } == -1
    {
        error!("yabai-msg: failed to send data..\n");
    }

    unsafe { libc::shutdown(socket_file_descriptor, libc::SHUT_WR) };
    drop(message);

    let mut result = libc::EXIT_SUCCESS;
    let mut standard_output = std::io::stdout();
    let mut standard_error = std::io::stderr();
    let mut output: &mut dyn Write = &mut standard_output;
    let mut bytes_read: isize;
    let mut response = [0u8; libc::BUFSIZ as usize];

    loop {
        bytes_read = unsafe {
            libc::read(
                socket_file_descriptor,
                response.as_mut_ptr().cast::<c_void>(),
                response.len() - 1,
            )
        };
        if !(bytes_read > 0) {
            break;
        }

        response[bytes_read as usize] = b'\0';

        if response[0] == FAILURE_RESPONSE_MARKER[0] {
            result = libc::EXIT_FAILURE;
            output = &mut standard_error;
            let text = &response[1..];
            let length = text
                .iter()
                .position(|byte| *byte == b'\0')
                .unwrap_or(text.len());
            let _ = output.write_all(&text[..length]);
            let _ = output.flush();
        } else {
            let text = &response[..];
            let length = text
                .iter()
                .position(|byte| *byte == b'\0')
                .unwrap_or(text.len());
            let _ = output.write_all(&text[..length]);
            let _ = output.flush();
        }
    }

    shut_down_and_close_socket(socket_file_descriptor);
    result
}
