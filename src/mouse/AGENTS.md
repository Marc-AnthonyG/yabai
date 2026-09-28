# mouse

Mouse support: the modifier and mode vocabulary, the state shared with the event tap, the drag
the event loop tracks while a window is moved or resized, and what a drop does to the tree.

## Notes

- The state is split in two. The half the tap reads is a static of atomics, written
  by the tap callback on the main thread and by configuration commands; the drag half is owned by
  `EventLoopOwnedState` and only the event-loop thread touches it.
- A consumed click is held as a retained `CGEvent` pointer in an atomic slot; whoever swaps it
  out releases it.
- Drop handling runs on the event-loop thread and takes the managers it touches as explicit
  parameters.
- The modifier and mode name tables are CLI spellings. The modifier table is indexed by the flag
  bit value, so its gaps are deliberate.
- The tap swallows a plain click, and its mouse-up, that lands on a visible group header, whether
  or not the window server would let it through to what lies under the header. The frames it
  checks are a mutex-guarded list the event loop replaces after every header refresh; the lock is
  never held across a call. The event loop turns a left click on a tab into focusing that window.
- A window dropped in the centre of a group, or anywhere on its header, stacks into that group
  whatever `mouse_drop_action` says, even when it is dragged out of another group.
