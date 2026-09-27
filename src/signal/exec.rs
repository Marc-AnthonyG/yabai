use core::ffi::c_char;
use std::ffi::{CStr, CString};

use crate::signal::definition::{SIGNAL_TYPE_COUNT, SIGNAL_TYPE_STR, Signal, SignalProp, SignalType};
use crate::signal::queue::PendingSignal;
use crate::support::regex::{RegexMatch, regex_match};

pub(crate) struct PreparedSignalCommand {
    arguments: Vec<CString>,
    environment: Vec<CString>,
}

fn regex_subject_truncated_at_the_first_null(subject: Option<&str>) -> CString {
    let bytes = subject.unwrap_or("").as_bytes();
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    CString::new(&bytes[..end]).unwrap_or_default()
}

pub(crate) fn event_signal_filter(event_signal: &PendingSignal, signal: &Signal) -> bool {
    match event_signal.signal_type {
        SignalType::ApplicationLaunched
        | SignalType::ApplicationActivated
        | SignalType::ApplicationDeactivated
        | SignalType::ApplicationVisible => {
            let regex_match_app = if signal.app_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            regex_match(
                signal.app_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.app.as_deref()),
            ) == regex_match_app
        }
        SignalType::ApplicationTerminated
        | SignalType::ApplicationHidden
        | SignalType::WindowDestroyed => {
            let regex_match_app = if signal.app_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            let app_no_match = regex_match(
                signal.app_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.app.as_deref()),
            ) == regex_match_app;

            let mut active = signal.active == SignalProp::Undefined;
            if !active {
                active = event_signal.active == i32::from(signal.active == SignalProp::Yes);
            }

            app_no_match || !active
        }
        SignalType::WindowCreated
        | SignalType::WindowFocused
        | SignalType::WindowDeminimized => {
            let regex_match_app = if signal.app_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            let app_no_match = regex_match(
                signal.app_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.app.as_deref()),
            ) == regex_match_app;

            let regex_match_title = if signal.title_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            let title_no_match = regex_match(
                signal.title_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.title.as_deref()),
            ) == regex_match_title;

            app_no_match || title_no_match
        }
        SignalType::WindowMoved
        | SignalType::WindowResized
        | SignalType::WindowMinimized
        | SignalType::WindowTitleChanged => {
            let regex_match_app = if signal.app_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            let app_no_match = regex_match(
                signal.app_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.app.as_deref()),
            ) == regex_match_app;

            let regex_match_title = if signal.title_regex_exclude {
                RegexMatch::Yes
            } else {
                RegexMatch::No
            };
            let title_no_match = regex_match(
                signal.title_regex.as_ref(),
                &regex_subject_truncated_at_the_first_null(event_signal.title.as_deref()),
            ) == regex_match_title;

            let mut active = signal.active == SignalProp::Undefined;
            if !active {
                active = event_signal.active == i32::from(signal.active == SignalProp::Yes);
            }

            app_no_match || title_no_match || !active
        }
        _ => false,
    }
}

pub(crate) fn event_signal_prepare_commands(
    signal_event: &[Vec<Signal>; SIGNAL_TYPE_COUNT],
    signal_storage: &[PendingSignal],
) -> Vec<PreparedSignalCommand> {
    let mut inherited_environment: Vec<CString> = Vec::new();
    unsafe {
        let mut entry = *libc::_NSGetEnviron();
        while !(*entry).is_null() {
            inherited_environment.push(CStr::from_ptr(*entry).to_owned());
            entry = entry.add(1);
        }
    }

    let mut prepared_commands: Vec<PreparedSignalCommand> = Vec::new();

    let count = signal_storage.len() as i32;

    for index in 0..count {
        let event_signal = &signal_storage[index as usize];

        let signal_count = signal_event[event_signal.signal_type as usize].len() as i32;
        crate::debug!(
            "{}: transmitting {} to {} subscriber(s)\n",
            "event_signal_flush",
            SIGNAL_TYPE_STR[event_signal.signal_type as usize],
            signal_count
        );

        for inner_index in 0..signal_count {
            let signal = &signal_event[event_signal.signal_type as usize][inner_index as usize];
            if event_signal_filter(event_signal, signal) {
                continue;
            }

            let mut environment = inherited_environment.clone();
            for argument in event_signal.arguments.iter() {
                if let Some((name, value)) = argument {
                    let entry = CString::new(format!("{}={}", name, value)).unwrap();
                    let prefix = format!("{}=", name);
                    let existing = environment
                        .iter()
                        .position(|entry| entry.as_bytes().starts_with(prefix.as_bytes()));
                    match existing {
                        Some(existing) => environment[existing] = entry,
                        None => environment.push(entry),
                    }
                }
            }

            let mut arguments = vec![
                CString::new("/usr/bin/env").unwrap(),
                CString::new("sh").unwrap(),
                CString::new("-c").unwrap(),
            ];
            if let Some(command) = signal.command.as_deref() {
                arguments.push(CString::new(command).unwrap());
            }

            prepared_commands.push(PreparedSignalCommand {
                arguments,
                environment,
            });
        }
    }

    prepared_commands
}

pub(crate) fn null_terminated_pointer_list(strings: &[CString]) -> Vec<*const c_char> {
    let mut pointers: Vec<*const c_char> = strings.iter().map(|string| string.as_ptr()).collect();
    pointers.push(core::ptr::null());
    pointers
}

pub(crate) fn event_signal_flush(
    signal_event: &mut [Vec<Signal>; SIGNAL_TYPE_COUNT],
    signal_storage: &mut Vec<PendingSignal>,
) {
    if signal_storage.is_empty() {
        return;
    }

    let prepared_commands = event_signal_prepare_commands(signal_event, signal_storage.as_slice());
    let prepared_command_pointers: Vec<(Vec<*const c_char>, Vec<*const c_char>)> =
        prepared_commands
            .iter()
            .map(|prepared_command| {
                (
                    null_terminated_pointer_list(&prepared_command.arguments),
                    null_terminated_pointer_list(&prepared_command.environment),
                )
            })
            .collect();

    let process_id = unsafe { libc::fork() };
    if process_id != 0 {
        signal_storage.clear();
        return;
    }

    for (argument_pointers, environment_pointers) in prepared_command_pointers.iter() {
        let process_id = unsafe { libc::fork() };
        if process_id != 0 {
            continue;
        }

        unsafe {
            *libc::_NSGetEnviron() = environment_pointers.as_ptr() as *mut *mut c_char;
            libc::_exit(libc::execvp(
                argument_pointers[0],
                argument_pointers.as_ptr(),
            ));
        }
    }

    unsafe { libc::_exit(libc::EXIT_SUCCESS) }
}
