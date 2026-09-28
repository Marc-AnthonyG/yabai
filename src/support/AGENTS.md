# support

The bottom layer of the daemon: small building blocks that know nothing of windows, spaces or
displays as state. Text, JSON, geometry and arithmetic helpers, files, the pattern filter rules
and signals share, the response the untyped command handlers write, logging and user
notifications, which macOS version the daemon runs on, the handles that name windows, processes,
spaces, displays and tree nodes, and the small value vocabularies (window layers, directions, resize handles, easing curves,
colours) that several modules share.

## Notes

- Nothing here depends on a manager or on `EventLoopOwnedState`; the only crate modules it
  reaches are `ffi` and the process-wide statics. Any thread may call into it.
- The window sub-layer and the easing curves carry their command-line and JSON spellings as clap
  and serde derives; a sub-layer turns into the layer number the window server takes.
- The response collects standard output until the first failure is written, then everything as
  failure text split into one failure per line; a silent response keeps no failure.
- `error!` exits with a failure status and `require!` with success, so launchd does not restart the
  daemon after a `require!`; which one a call site uses is deliberate.
- Handles are plain ids looked up at each use, never pointers. The tree root handle
  is `NodeId` 0.
- The macOS version flags are written once at start-up, when the workspace observer is created,
  and only read afterwards.
- The spawner reads a child's exit status even while the process ignores SIGCHLD, when the
  system reaps children itself and `waitpid` only fails with `ECHILD`. It spawns the program
  suspended, watches its exit on a kqueue with `NOTE_EXITSTATUS`, then resumes it, so the exit
  cannot slip past the watch; a `WNOHANG` reap afterwards covers SIGCHLD at its default. A
  program ended by a signal, or one that cannot be spawned, has no exit status.
- A file change watch is on the file itself, reached through any symlink, not on its path. A save
  that renames a new file over it ends the watch with a delete, and only a new watch sees the file
  now at that path.
