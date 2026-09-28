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
- The order of `launchctl` subcommands (print, then enable and bootstrap, or kickstart; print,
  then kill, or bootout and disable) is the launchd state machine yabai relies on; each branch
  is observable to the user's launchd domain.
