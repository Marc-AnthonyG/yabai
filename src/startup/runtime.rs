pub(crate) fn install_panic_hook_that_aborts_the_process() {
    std::panic::set_hook(Box::new(|panic_information| {
        eprintln!("{panic_information}");
        std::process::abort();
    }));
}

pub(crate) fn restore_default_sigpipe_disposition() {
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
}
