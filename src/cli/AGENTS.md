# cli

The command line: parsing yabai's options, the one-shot options that act and exit (the
scripting-addition and launchd service commands, `--help`, `--version`), and the `-m` client
that sends a message to the running daemon and prints its reply.

## Notes

- Option spellings, the help and version text and the exit codes are the public CLI. A one-shot option exits with the status of the action it runs.
- The client's message is a native-endian `int` length, then every argument null-terminated, then
  one more null; it goes to the per-user socket path formatted from `$USER`.
- The client checks every chunk it reads for the failure prefix byte. A chunk that
  starts with it is printed to stderr without that byte, everything after it goes to stderr too,
  and the client exits with failure. This per-chunk check is the C behaviour and stays.
- Everything here runs on the main thread before any other thread exists. Parsing either exits
  the process or stores the config file path once for start-up.
