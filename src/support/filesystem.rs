pub fn can_owner_execute_file(filename: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(filename) {
        Ok(buffer) => (buffer.permissions().mode() & libc::S_IXUSR as u32) != 0,
        Err(_) => false,
    }
}
