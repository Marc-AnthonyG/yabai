#[macro_export]
macro_rules! debug {
    ($($argument:tt)*) => {{
        if $crate::support::log::g_verbose() {
            print!($($argument)*);
        }
    }};
}

#[macro_export]
macro_rules! warn {
    ($($argument:tt)*) => {{
        eprint!($($argument)*);
    }};
}

#[macro_export]
macro_rules! error {
    ($($argument:tt)*) => {{
        eprint!($($argument)*);
        std::process::exit(libc::EXIT_FAILURE)
    }};
}

#[macro_export]
macro_rules! require {
    ($($argument:tt)*) => {{
        eprint!($($argument)*);
        std::process::exit(libc::EXIT_SUCCESS)
    }};
}

pub fn g_verbose() -> bool {
    crate::state::process_wide::VERBOSE.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_g_verbose(value: bool) {
    crate::state::process_wide::VERBOSE.store(value, std::sync::atomic::Ordering::Relaxed);
}

pub fn or_null(value: Option<&str>) -> &str {
    match value {
        Some(text) => text,
        None => "(null)",
    }
}

pub fn debug_message(prefix: &str, message: &str) {
    if !g_verbose() {
        return;
    }

    print!("{}:", prefix);
    for token in message.split('\0') {
        if token.is_empty() {
            break;
        }
        print!(" {}", token);
    }
    print!("\n");
    let _ = std::io::Write::flush(&mut std::io::stdout());
}
