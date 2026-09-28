# protocol

What a client and the running window manager exchange over the per-user socket
`/tmp/yabai_$USER.socket`: one request, one reply, then the connection closes.

## Notes

- A request is a native-endian `u32` byte length, then the JSON of `DaemonRequest`. The length
  keeps the framing an older daemon reads, so such a daemon answers with a failure of its own
  instead of allocating a length decoded from JSON text; the client cannot read that answer and
  exits 3.
- The daemon reads `client_version` before it decodes the command, and refuses a client whose
  Cargo.toml version is not its own. Two builds of one version are not told apart: a command
  the daemon does not know fails to decode, and a field it does not know is ignored.
- A reply is the JSON of `DaemonReply`, and the daemon closes the stream after it, so end of file
  delimits it. The client prints `standard_output` verbatim and each failure as one stderr line.
- The daemon writes a reply in one call: the pretty JSON inside it escapes into many small pieces,
  which would each become a write on the event-loop thread.
- A client exits 0 on success, 1 when the window manager reports a failure, 2 on a usage error
  (clap's status) and 3 when no running window manager answers in a way it understands.
