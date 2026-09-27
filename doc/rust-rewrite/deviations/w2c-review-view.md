# Deviations — `w2c-review-view` (fidelity review of `src/view.rs` and `src/space_manager.rs`)

One line each: C location | what C did | what Rust does.

`src/view.c:15`, `:107`, `:114-117` | a feedback window whose `SLSNewWindowWithOpaqueShapeAndContext` left the id `0` was created again on the next `insert_feedback_show` and skipped by `insert_feedback_destroy`, and its `CGContextRef` was leaked when overwritten or when the node was freed | both functions test the window id exactly as C does, so creation is retried again and the `w2-view-part1` line for `:15` no longer holds; `FeedbackWindow::drop` of an id-`0` window makes no SkyLight call and only releases the context C leaked (`DECISIONS.md` 4)

`src/view.c:43`, `:108`, `:345`, `:718-719` | `insert_feedback_destroy` removed the entry keyed by the node's current `window_order[0]`, so an entry added under an earlier front window (one removed from the stack at `:630-648`) survived `free(node)` and kept pointing at freed memory, which `event_loop.c:28` and `:948` read | `view_free_node` also removes every `insert_feedback` entry whose value names the freed `(SpaceId, NodeId)`, so no entry can resolve to a reused slot (`DECISIONS.md` 4)

`src/view.c:682` | `parent->feedback_window = child->feedback_window` overwrote, and leaked, an overlay the parent still carried from before it was split (a leaf split while it held the mouse-drag feedback) | assigning the `Option<FeedbackWindow>` drops the parent's old overlay, ordering it out and releasing its window and context (`DECISIONS.md` 4)

`src/space_manager.c:1190` | when a matched view's new sid was still held by another, unmatched view, `table_add` did not overwrite and the removed `struct view` was leaked while `managed_window` kept pointing at it | `Table::add` drops the `View`, whose `FeedbackWindow`s release their overlays, and its `managed_window` / `insert_feedback` handles keep the old `SpaceId`, which resolves to a miss (`DECISIONS.md` 4); WindowServer does not reuse space ids, so no run reaches this
