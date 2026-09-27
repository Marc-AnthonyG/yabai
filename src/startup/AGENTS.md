# startup

Bringing the daemon up: the panic hook, the checks that refuse to run, the process-wide settings
and the lock file, the start-up order that builds the event-loop state and begins every manager
and observer before the event loop starts, and running the user's config file.

## Notes

- The order is behaviour (decision 12). The state is built on the main thread in the C start-up
  order and moved into the event-loop thread, which is spawned only once the window manager has
  begun and window notifications are requested. The event channel exists before any of it, so
  events posted during start-up wait in order. The message socket, the config file and the main
  run loop come last. Which notifications are registered depends on the macOS version, as in C.
- A panic prints and aborts the whole process (decision 7). SIGPIPE is put back to its default
  before the command line is parsed, because the Rust runtime ignores it before `main`; the
  daemon then ignores SIGCHLD and SIGPIPE itself, as C did, and its children inherit that
  (decision 25).
- The `OnceLock` statics are written once, before any other thread starts (decision 18). On the
  daemon path the command line stores the config file path and every other one is set here.
- The lock file is opened, write-locked with `fcntl` and its descriptor never closed
  (decision 34); failing to take the lock means another instance is running. The `/tmp` socket
  and lock paths are formatted from `$USER` and are observable (decision 3).
- A missing requirement (running as root, no accessibility access, displays without separate
  spaces) exits with `EXIT_SUCCESS` so launchd does not restart the daemon; every other start-up
  failure exits with `EXIT_FAILURE` (decision 33).
- Unless `-c` names one, the config file is the first of `$XDG_CONFIG_HOME/yabai/yabairc`,
  `~/.config/yabai/yabairc` and `~/.yabairc` that exists. It runs in a forked child through
  `/usr/bin/env sh`, with `-c` when the file is executable (decisions 3, 25).
