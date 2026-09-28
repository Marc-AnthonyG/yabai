pub fn is_existing_directory(filename: &str) -> bool {
    match std::fs::metadata(filename) {
        Ok(buffer) => buffer.is_dir(),
        Err(_) => false,
    }
}

pub fn is_existing_file_that_is_not_a_directory(filename: &str) -> bool {
    match std::fs::metadata(filename) {
        Ok(buffer) => !buffer.is_dir(),
        Err(_) => false,
    }
}

pub fn can_owner_execute_file(filename: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(filename) {
        Ok(buffer) => (buffer.permissions().mode() & libc::S_IXUSR as u32) != 0,
        Err(_) => false,
    }
}
