# startup

Bringing the daemon up: the panic hook, the checks that refuse to run, asking for the macOS
permissions and waiting for Accessibility, the process-wide settings and the lock file, the
watcher that relaunches the daemon once a Screen Recording grant arrives, the start-up order that
builds the event-loop state and begins every manager and observer before the event loop starts,
and running the user's config file once the message socket listens.

## Notes

- The order is behaviour. The state is built on the main thread in the C start-up
  order and moved into the event-loop thread, which is spawned only once the window manager has
  begun and window notifications are requested. The event channel exists before any of it, so
  events posted during start-up wait in order. The message socket, the config file and the main
  run loop come last. Which notifications are registered depends on the macOS version, as in C.
- A panic prints and aborts the whole process. SIGPIPE is put back to its default
  before the command line is parsed, because the Rust runtime ignores it before `main`; the
  daemon then ignores SIGCHLD and SIGPIPE itself, as C did, and its children inherit that.
- The `OnceLock` statics are written once, before any other thread starts. On the
  daemon path the command line stores the config file path and every other one is set here.
- The lock file is opened, write-locked with `fcntl` and its descriptor never closed; failing to take the lock means another instance is running. The `/tmp` socket
  and lock paths are formatted from `$USER` and are observable.
- A missing requirement (running as root, displays without separate spaces) exits with
  `EXIT_SUCCESS` so launchd does not restart the daemon; every other start-up failure exits with
  `EXIT_FAILURE`. Missing Accessibility access is not an exit: it is waited for.
- Every missing permission is asked for at once, right after the root check and before anything
  waits, so the system dialogs appear together: the Accessibility prompt when untrusted, and a
  Screen Recording request when it is not granted and System Integrity Protection is relaxed
  enough for the scripting addition, since window animations need both. The separate-spaces check
  comes next and can still exit. An untrusted daemon then says once on stderr that it is waiting
  and polls `AXIsProcessTrusted`, without the prompt option, every 500 ms until it is trusted, and
  start-up carries on in the same process.
- A running process never sees a Screen Recording grant made after it started. When one was
  requested, a watcher thread starts once the lock is held, so every process-wide static is set
  and SIGCHLD is already ignored. It runs the yabai binary itself (`current_exe`) with the unlisted
  `--report-screen-recording-permission` two seconds apart, for at most ten minutes, then stops
  quietly. On a grant it relaunches the daemon: `launchctl kickstart -k` when `XPC_SERVICE_NAME`
  names the yabai service, otherwise, or when the kickstart fails, one stderr line asks the user
  to restart yabai. It touches no event-loop state and does nothing else.
- Because the daemon ignores SIGCHLD, `waitpid` cannot read a child's exit status; the watcher's
  children are read through the support spawner, which waits on a kqueue instead.
