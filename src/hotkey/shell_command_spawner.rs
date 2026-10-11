use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender, channel};

use libc::qos_class_t;

use crate::ffi::libsystem::set_the_quality_of_service_class_of_this_thread;
use crate::hotkey::chord::Chord;
use crate::support::shell_command::spawn_shell_running_the_command_without_waiting_for_it;

pub(crate) struct ShellCommandOfAPressedHotkey {
    pub(crate) chord: Chord,
    pub(crate) shell_command: String,
}

static SENDER_TO_THE_SHELL_HOTKEY_SPAWNER_THREAD: OnceLock<Sender<ShellCommandOfAPressedHotkey>> =
    OnceLock::new();

pub(crate) fn start_the_thread_that_spawns_shell_hotkeys() -> std::io::Result<()> {
    let (sender, receiver) = channel();
    std::thread::Builder::new()
        .name(String::from("yabai-shell-hotkey-spawner"))
        .spawn(move || {
            spawn_each_shell_command_handed_over_for_as_long_as_the_daemon_runs(receiver)
        })?;
    let _ = SENDER_TO_THE_SHELL_HOTKEY_SPAWNER_THREAD.set(sender);
    Ok(())
}

pub(crate) fn hand_the_shell_command_to_the_spawner_thread(
    shell_command_of_a_pressed_hotkey: ShellCommandOfAPressedHotkey,
) {
    if let Some(sender) = SENDER_TO_THE_SHELL_HOTKEY_SPAWNER_THREAD.get() {
        let _ = sender.send(shell_command_of_a_pressed_hotkey);
    }
}

fn spawn_each_shell_command_handed_over_for_as_long_as_the_daemon_runs(
    receiver: Receiver<ShellCommandOfAPressedHotkey>,
) {
    set_the_quality_of_service_class_of_this_thread(qos_class_t::QOS_CLASS_USER_INITIATED);
    for ShellCommandOfAPressedHotkey {
        chord,
        shell_command,
    } in receiver
    {
        let shell = spawn_shell_running_the_command_without_waiting_for_it(
            &shell_command,
            std::iter::empty::<(&str, &str)>(),
        );
        if let Err(error) = shell {
            crate::warn!("yabai: hotkey '{chord}' could not run its shell command: {error}\n");
        }
    }
}
