# mouse

Mouse support: the modifier and mode vocabulary, the state shared with the event tap, the drag
the event loop tracks while a window is moved or resized, and what a drop does to the tree.

## Notes

- The state is split in two (decision 23). The half the tap reads is a static of atomics, written
  by the tap callback on the main thread and by configuration commands; the drag half is owned by
  `EventLoopOwnedState` and only the event-loop thread touches it.
- A consumed click is held as a retained `CGEvent` pointer in an atomic slot; whoever swaps it
  out releases it.
- Drop handling runs on the event-loop thread and takes the managers it touches as explicit
  parameters (decision 13).
- The modifier and mode name tables are CLI spellings. The modifier table is indexed by the flag
  bit value, so its gaps are deliberate (decision 31).
