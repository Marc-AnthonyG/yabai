# layout

The tiling layout of each space: the view a space owns, its BSP tree of window nodes, the area
arithmetic that divides the display between them and predicts the frame an inserted window will
take, the per-view settings that override the global ones, and the insertion point with its
preview, a SkyLight overlay window drawn as a ghost of that frame.

## Notes

- Everything here runs on the event-loop thread and takes the managers it touches as explicit
  parameters. Nodes are named by `(space id, NodeId)` and looked up at each use; a missing view is an early return.
- The tree is an index arena owned by its view. The root is always `NodeId` 0
  because the C reset the root in place instead of reallocating it; do not free or move it.
- Freed node ids are recycled. Before a node is freed, every reference to it outside the arena
  (the window manager's insert-feedback table, the mouse drag state) is scrubbed, so a stale
  `NodeId` can never name a reused node.
- A node's feedback window is a SkyLight window owned by the node and released when the node's
  feedback slot is dropped.
- Areas are `f32`. The truncations and the `+ 0.5` rounding in the split arithmetic
  place windows to the pixel and are observable.
- The name tables for view layout, split, child, auto-balance and insertion point are the CLI and
  query spellings; each index is the enum discriminant.
- The preview is computed by one pure function from the node's area, the insert
  direction, the node's ratio or the global split ratio, and the view's gap, with the same split
  arithmetic the tree runs when it inserts: `window --insert` and a mouse drop both use it, and a
  swap, a stack or any insertion into a view with the stack layout covers the whole node. It shows
  the frame the split gives; auto-balance may still move frames once the window is inserted. Keep
  it in step with the tree whenever the split rules change.
- The overlay covers only that frame and is drawn at a 2.0 resolution so it stays sharp on Retina
  displays. It fades in through steps posted to the event loop from the main queue; a step only
  touches overlay windows a node still owns, so a released overlay is never touched, and a step is
  scheduled only while some overlay is still fading in and no other step is pending, so replacing
  one overlay with another never starts a second chain of steps.
- The preview colour is the window manager's insert feedback colour. It follows the accent colour
  the workspace observer reads and posts, until `insert_feedback_color` is set by a client.
- A pending insertion point is cleared when a window yabai tracks gains focus and is not that
  insertion point, the way repeating the same `--insert` clears it. A window that is
  not tracked yet cannot clear it, so the new window still consumes it. A node that is showing the
  mouse drag preview keeps its overlay and insert direction; the drag owns them.
