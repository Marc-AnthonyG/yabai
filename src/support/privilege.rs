pub fn is_running_as_root() -> bool {
    unsafe { libc::getuid() == 0 || libc::geteuid() == 0 }
}
