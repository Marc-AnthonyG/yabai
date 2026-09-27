# layout

The tiling layout of each space: the view a space owns, its BSP tree of window nodes, the area
arithmetic that divides the display between them, the per-view settings that override the global
ones, and the insertion point with its feedback window.

## Notes

- Everything here runs on the event-loop thread and takes the managers it touches as explicit
  parameters (decision 13). Nodes are named by `(space id, NodeId)` and looked up at each use
  (decision 14); a missing view is an early return.
- The tree is an index arena owned by its view (decision 15). The root is always `NodeId` 0
  because the C reset the root in place instead of reallocating it; do not free or move it.
- Freed node ids are recycled. Before a node is freed, every reference to it outside the arena
  (the window manager's insert-feedback table, the mouse drag state) is scrubbed, so a stale
  `NodeId` can never name a reused node.
- A node's feedback window is a SkyLight window owned by the node and released when the node's
  feedback slot is dropped.
- Areas are `f32` (decision 30). The truncations and the `+ 0.5` rounding in the split arithmetic
  place windows to the pixel and are observable (decision 3).
- The name tables for view type, split, child, auto-balance and insertion point are the CLI and
  query spellings; each index is the enum discriminant (decision 31).
