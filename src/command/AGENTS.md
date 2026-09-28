# command

The command line as clap types: the options of a bare `yabai`, which runs the window manager, the
local `service` and `scripting-addition` actions, and every command sent to the running window
manager, which crosses the socket as JSON.

## Notes

- `///` comments on clap items become the generated `--help` and man page. They are public
  surface; nothing else here carries doc comments.
- Every type that crosses the socket derives `Serialize` and `Deserialize`. Client and daemon are
  the same binary and the protocol refuses a client of another version, so the serde shape only
  has to agree with itself.
- Values are checked while parsing, so a bad value is a usage error before anything is sent.
  Selectors are only parsed here; the daemon resolves them against live state.
- The hidden `-m` passes an old argument vector on as `DaemonCommand::NotYetTyped`, for the
  domains that are not typed yet. It is a bridge, not a second grammar: nothing new goes through
  it.
