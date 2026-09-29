# command

The command line as clap types: the options of a bare `yabai`, which runs the window manager, the
local `service` and `scripting-addition` actions, and every command sent to the running window
manager, which crosses the socket as JSON.

## Notes

- `///` comments on clap items become the generated `--help` and man page. They are public
  surface; nothing else here carries doc comments. What fits no clap item (selectors, output,
  exit status) is the top-level long help after the options, read from a text file whose lines
  ending in `:` at column 0 become man page sections.
- `doc/yabai.1` is generated from the tree by `just man`; a test fails while it differs from what
  the tree generates, so change the tree, then regenerate.
- Every type that crosses the socket derives `Serialize` and `Deserialize`. Client and daemon are
  the same binary and the protocol refuses a client of another version, so the serde shape only
  has to agree with itself.
- Values are checked while parsing, so a bad value is a usage error before anything is sent.
  Selectors are only parsed here; the daemon resolves them against live state.
- The acting display, space or window is a global `-d`, `-s` or `-w` option, so it may follow
  the action. A display or space label is refused when the selector of its kind would read it as
  anything else.
