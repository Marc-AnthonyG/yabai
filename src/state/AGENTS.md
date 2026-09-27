# state

The daemon's state that belongs to no single manager: the struct bundling everything the
event-loop thread owns, the Mission Control mode it tracks, and the process-wide statics any
thread may read.

## Notes

- `EventLoopOwnedState` is built on the main thread at start-up, moved into the event-loop
  thread and touched by nothing else afterwards (decision 12). It is `Send` through an
  `unsafe impl` argued in `THREADS.md`. Its managers are passed on explicitly, never reached
  through a global (decision 13).
- The process-wide statics are `OnceLock`s written once before any other thread starts
  (decision 18). The exceptions are atomics: the verbose flag, and the pending-focus,
  pending-gesture, last-gesture and last cmd-tab stamps that main-thread callbacks write and the
  event loop reads.
- The Mission Control mode names are what signals export in `YABAI_MISSION_CONTROL_MODE`
  (decision 3), indexed by the mode's discriminant (decision 31).
