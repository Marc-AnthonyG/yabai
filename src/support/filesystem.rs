pub fn directory_exists(filename: &str) -> bool {
    match std::fs::metadata(filename) {
        Ok(buffer) => buffer.is_dir(),
        Err(_) => false,
    }
}

pub fn file_exists(filename: &str) -> bool {
    match std::fs::metadata(filename) {
        Ok(buffer) => !buffer.is_dir(),
        Err(_) => false,
    }
}

pub fn file_can_execute(filename: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(filename) {
        Ok(buffer) => (buffer.permissions().mode() & libc::S_IXUSR as u32) != 0,
        Err(_) => false,
    }
}
