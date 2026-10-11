use std::ffi::OsStr;
use std::process::Command;

pub(crate) fn spawn_shell_running_the_command_without_waiting_for_it<VariableName, VariableValue>(
    command: &str,
    variables_added_to_its_environment: impl IntoIterator<Item = (VariableName, VariableValue)>,
) -> std::io::Result<()>
where
    VariableName: AsRef<OsStr>,
    VariableValue: AsRef<OsStr>,
{
    Command::new("/bin/sh")
        .args(["-c", command])
        .envs(variables_added_to_its_environment)
        .spawn()
        .map(drop)
}
