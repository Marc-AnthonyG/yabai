use std::io::Write;

#[macro_export]
macro_rules! debug {
    ($($argument:tt)*) => {{
        if $crate::support::log::g_verbose() {
            let _ = std::io::Write::write_fmt(
                &mut std::io::stdout(),
                format_args!($($argument)*),
            );
        }
    }};
}

#[macro_export]
macro_rules! warn {
    ($($argument:tt)*) => {{
        let _ = std::io::Write::write_fmt(
            &mut std::io::stderr(),
            format_args!($($argument)*),
        );
    }};
}

#[macro_export]
macro_rules! error {
    ($($argument:tt)*) => {{
        let _ = std::io::Write::write_fmt(
            &mut std::io::stderr(),
            format_args!($($argument)*),
        );
        std::process::exit(libc::EXIT_FAILURE)
    }};
}

#[macro_export]
macro_rules! require {
    ($($argument:tt)*) => {{
        let _ = std::io::Write::write_fmt(
            &mut std::io::stderr(),
            format_args!($($argument)*),
        );
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

    let mut standard_output = std::io::stdout().lock();
    let _ = write!(standard_output, "{}:", prefix);
    for token in message.split('\0') {
        if token.is_empty() {
            break;
        }
        let _ = write!(standard_output, " {}", token);
    }
    let _ = standard_output.write_all(b"\n");
    let _ = standard_output.flush();
}
