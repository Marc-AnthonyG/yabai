# layout

The tiling layout of each space: the view a space owns, its BSP tree of window nodes, the area
arithmetic that divides the display between them and predicts the frame an inserted window will
take, the per-view settings that override the global ones, and the insertion point with its
preview, a SkyLight overlay window drawn as a ghost of that frame.

## Notes

- Everything here runs on the event-loop thread and takes the managers it touches as explicit
  parameters. Nodes are named by `(space id, NodeId)` and looked up at each use; a missing view is an early return.
- The tree is an index arena owned by its view. The root is always `NodeId` 0 and is reset in
  place; do not free or move it.
- Freed node ids are recycled. Before a node is freed, every reference to it outside the arena
  (the window manager's insert-feedback table, the mouse drag state) is scrubbed, so a stale
  `NodeId` can never name a reused node.
- A node's feedback window is a SkyLight window owned by the node and released when the node's
  feedback slot is dropped.
- The view layout, split, child and insertion point enums carry their command-line and JSON
  spellings as clap and serde derives; the "none" and "default" variants are never accepted on
  the command line.
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
  the workspace observer reads and posts, until a client sets `--insert-feedback-color`.
- A pending insertion point is cleared when a window yabai tracks gains focus and is not that
  insertion point, the way repeating the same `--insert` clears it. A window that is
  not tracked yet cannot clear it, so the new window still consumes it. A node that is showing the
  mouse drag preview keeps its overlay and insert direction; the drag owns them.
- A group is a leaf holding stacked windows. In bsp a leaf with more than one window is always a
  group; a leaf with one window is a group while that window's `stays_a_group_on_its_own` mark is
  set. The mark is set when a lone window is grouped and when a group is left with one window, and
  cleared when a window leaves a multi-window leaf or its group is ungrouped. It travels with the
  window, so a lone group warped, minimised or sent to another space is still a group where it lands.
  In the stack layout the root is always a group.
- A group's windows get its tile (or zoom) area minus the header, whose height is clamped to half
  the tile; `node.area` stays the full tile because sibling split arithmetic reads it. Every place
  that turns a tile into window frames, and the moved/resized handlers' drift checks, go through
  `group_area`, or a group's windows would be pulled back to the full tile after every move.
- `--layout` to the layout a view already has does nothing. Leaving bsp snapshots the view's
  multi-window groups on the view; the next bsp tiling stacks a returning member onto the tile of
  a member already placed, then restores stack order and the front window, and forgets the snapshot.
- A group header is a SkyLight window owned by its node, like the insert overlay. Headers are
  rebuilt from the tree by one refresh of the whole view, run whenever windows are moved into
  their areas, a window leaves a group, focus or a title changes, a space becomes visible and after
  Mission Control; only a visible space is refreshed, and a new header is moved onto its view's
  space. They hide during Mission Control, snap to their final frame instead of animating, and
  publish their frames to the mouse tap after every refresh.
- Tab titles are CoreText lines drawn into the header's SkyLight context, whose y axis points up;
  tab frames are flipped to screen coordinates for hit tests.
