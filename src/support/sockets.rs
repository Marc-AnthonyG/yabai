use core::ffi::c_char;

pub fn open_unix_stream_socket(socket_file_descriptor: &mut i32) -> bool {
    *socket_file_descriptor = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    *socket_file_descriptor != -1
}

pub fn connect_socket_to_unix_path(socket_file_descriptor: i32, socket_path: &str) -> bool {
    let mut socket_address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    socket_address.sun_family = libc::AF_UNIX as libc::sa_family_t;

    let socket_path_bytes = socket_path.as_bytes();
    let writable_length = socket_address.sun_path.len() - 1;
    let copied_length = socket_path_bytes.len().min(writable_length);
    for index in 0..copied_length {
        socket_address.sun_path[index] = socket_path_bytes[index] as c_char;
    }

    unsafe {
        libc::connect(
            socket_file_descriptor,
            (&socket_address as *const libc::sockaddr_un).cast::<libc::sockaddr>(),
            std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        ) != -1
    }
}

pub fn shut_down_and_close_socket(socket_file_descriptor: i32) {
    unsafe {
        libc::shutdown(socket_file_descriptor, libc::SHUT_RDWR);
        libc::close(socket_file_descriptor);
    }
}
