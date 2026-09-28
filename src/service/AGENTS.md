# service

The launchd user agent that runs yabai: the plist file that describes it, and the `launchctl`
invocations that install, start, restart and stop it.

## Notes

- This code runs on the command-line path, on the main thread, before any daemon state exists.
  Every entry point there returns the process exit code; a fatal error ends the process with
  `EXIT_FAILURE`. The one exception is the daemon relaunching itself from the Screen Recording
  grant watcher thread: telling whether `XPC_SERVICE_NAME` names the yabai service, and
  `launchctl kickstart -k` on it. That kickstart never exits the process and reads launchctl's
  exit status through the support spawner, since the daemon ignores SIGCHLD by then.
- The plist text is externally observable: the label, the program path, the `PATH`
  it captures, the `/tmp/yabai_<user>.out.log` and `.err.log` paths, `KeepAlive`, `ProcessType`
  and `Nice`. Its whitespace, including the tab-indented lines, is reproduced from the C and is
  not to be tidied.
- `launchctl` is spawned with `posix_spawn` and a NULL environment, and the executable path comes
  from `_NSGetExecutablePath`.
- The order of `launchctl` subcommands (print, then enable and bootstrap, or kickstart; print,
  then kill, or bootout and disable) is the launchd state machine yabai relies on; each branch
  is observable to the user's launchd domain.
