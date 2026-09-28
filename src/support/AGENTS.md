# support

The bottom layer of the daemon: small building blocks that know nothing of windows, spaces or
displays as state. Text, JSON, geometry and arithmetic helpers, sockets and files, the
C-compatible hash table, POSIX regex, the client response, logging and user notifications, which
macOS version the daemon runs on, the handles that name windows, processes, spaces, displays and
tree nodes, and the small value vocabularies (window layers, directions, resize handles, easing
curves, colours) that several modules share.

## Notes

- Nothing here depends on a manager or on `EventLoopOwnedState`; the only crate modules it
  reaches are `ffi` and the process-wide statics. Any thread may call into it.
- Text built here reaches clients byte for byte: the failure prefix byte, JSON fragments and
  string escaping, and the on/off, layer and easing names that replies print and
  the CLI parses. Layer and easing values index their name tables. None of it may
  be respelled, reordered or renumbered.
- The hash table keeps the C hash functions, bucket iteration order and the "add does not
  overwrite" rule; the order of query output depends on it.
- The response owns the failure prefix and the "no response wanted" case. The
  regex match stays three-valued.
- `error!` exits with a failure status and `require!` with success, so launchd does not restart the
  daemon after a `require!`; which one a call site uses is deliberate.
- Handles are plain ids looked up at each use, never pointers. The tree root handle
  is `NodeId` 0.
- The alpha restore uses the SSE2 intrinsics on x86_64 and the NEON ones on arm64, instruction for
  instruction, so both targets produce the same pixels as the C.
- The macOS version flags are written once at start-up, when the workspace observer is created,
  and only read afterwards. Which version-specific path runs is behaviour.
- The spawner reads a child's exit status even while the process ignores SIGCHLD, when the
  system reaps children itself and `waitpid` only fails with `ECHILD`. It spawns the program
  suspended, watches its exit on a kqueue with `NOTE_EXITSTATUS`, then resumes it, so the exit
  cannot slip past the watch; a `WNOHANG` reap afterwards covers SIGCHLD at its default. A
  program ended by a signal, or one that cannot be spawned, has no exit status.
