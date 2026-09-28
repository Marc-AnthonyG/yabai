# cli

What the command line runs outside the window manager: the client that sends a command to the
running window manager and prints its reply, the local `service` and `scripting-addition`
actions, and the unlisted Screen Recording permission report.

## Notes

- `--report-screen-recording-permission` is deliberately unlisted: it stays out of `--help`, the
  manual and the README. The daemon runs its own binary with it because a running process never
  sees a Screen Recording grant made after it started. It prints nothing and exits 0 when
  `CGPreflightScreenCaptureAccess` is true, 1 otherwise. Its spelling and exit statuses are a
  contract with a running daemon whose binary may since have been replaced by an older or newer
  one; an unknown option exits 2, which the daemon also reads as not granted.
- A local action exits with the status of the action it runs.
- Everything here runs on the main thread before any other thread exists.
