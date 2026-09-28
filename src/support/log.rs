#[macro_export]
macro_rules! debug {
    ($($argument:tt)*) => {{
        if $crate::support::log::is_verbose_debug_output_enabled() {
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

pub fn is_verbose_debug_output_enabled() -> bool {
    crate::state::process_wide::VERBOSE_DEBUG_OUTPUT_ENABLED
        .load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_verbose_debug_output_enabled(value: bool) {
    crate::state::process_wide::VERBOSE_DEBUG_OUTPUT_ENABLED
        .store(value, std::sync::atomic::Ordering::Relaxed);
}

pub fn text_or_printf_null_placeholder(value: Option<&str>) -> &str {
    match value {
        Some(text) => text,
        None => "(null)",
    }
}
