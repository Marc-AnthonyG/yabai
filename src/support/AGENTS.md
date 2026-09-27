# support

The bottom layer of the daemon: small building blocks that know nothing of windows, spaces or
displays as state. Text, JSON, geometry and arithmetic helpers, sockets and files, the
C-compatible hash table, POSIX regex, the client response, logging and user notifications, which
macOS version the daemon runs on, the handles that name windows, processes, spaces, displays and
tree nodes, and the small value vocabularies (window layers, directions, resize handles, easing
curves, colours) that several modules share.

## Notes

- Nothing here depends on a manager or on `EventLoopOwnedState`; the only crate modules it
  reaches are `ffi` and the process-wide statics (decision 18). Any thread may call into it.
- Text built here reaches clients byte for byte: the failure prefix byte, JSON fragments and
  string escaping (decision 29), and the on/off, layer and easing names that replies print and
  the CLI parses. Layer and easing values index their name tables (decision 31). None of it may
  be respelled, reordered or renumbered (decision 3).
- The hash table keeps the C hash functions, bucket iteration order and the "add does not
  overwrite" rule (decision 16); the order of query output depends on it.
- The response owns the failure prefix and the "no response wanted" case (decisions 28, 43). The
  regex match stays three-valued (decisions 26, 44).
- The exit codes of the fatal log macros are decision 33; which one a call site uses is
  deliberate.
- Handles are plain ids looked up at each use, never pointers (decision 14). The tree root handle
  is `NodeId` 0 (decision 15).
- The alpha restore follows decision 36 on each target.
- The macOS version flags are written once at start-up, when the workspace observer is created,
  and only read afterwards (decision 18). Which version-specific path runs is behaviour.
