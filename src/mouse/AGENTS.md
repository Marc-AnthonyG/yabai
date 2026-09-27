# mouse

Mouse support: the CGEventTap that sees clicks, drags, moves and Dock swipe gestures, the state
shared with it, the drag the event loop tracks while a window is moved or resized, and what a
drop does to the tree.

## Notes

- The state is split in two (decision 23). The half the tap reads is a static of atomics; the
  drag half is owned by `EventLoopOwnedState` and only the event-loop thread touches it.
- The tap callback runs on the main thread. It reads and writes only the atomic half and the
  gesture statics, and hands everything else to the event loop as an event (decision 20).
- A consumed click is held as a retained `CGEvent` pointer in an atomic slot; whoever swaps it
  out releases it. The tap re-enables itself when macOS disables it by timeout or user input.
- Drop handling runs on the event-loop thread and takes the managers it touches as explicit
  parameters (decision 13).
- The modifier and mode name tables are CLI spellings. The modifier table is indexed by the flag
  bit value, so its gaps are deliberate (decision 31).
