# cli

The command line: parsing yabai's options, the one-shot options that act and exit (the
scripting-addition and launchd service commands, `--help`, `--version`, and the unlisted Screen
Recording permission report), and the `-m` client that sends a message to the running daemon and
prints its reply.

## Notes

- Option spellings, the help and version text and the exit codes are the public CLI. A one-shot option exits with the status of the action it runs.
- `--report-screen-recording-permission` is deliberately unlisted: it stays out of `--help`, the
  manual and the README. The daemon runs its own binary with it because a running process never
  sees a Screen Recording grant made after it started. It prints nothing and exits 0 when
  `CGPreflightScreenCaptureAccess` is true, 1 otherwise. Its spelling and exit statuses are a
  contract with a running daemon whose binary may since have been replaced by an older or newer
  one; an unknown option also exits 1, which the daemon reads as not granted.
- The client's message is a native-endian `int` length, then every argument null-terminated, then
  one more null; it goes to the per-user socket path formatted from `$USER`.
- The client checks every chunk it reads for the failure prefix byte. A chunk that
  starts with it is printed to stderr without that byte, everything after it goes to stderr too,
  and the client exits with failure. This per-chunk check is the C behaviour and stays.
- Everything here runs on the main thread before any other thread exists. Parsing either exits
  the process or stores the config file path once for start-up.
