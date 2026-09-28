# space

Spaces: the space manager's table of views and the global layout defaults, space labels, the
settings and tree operations applied to a space's view, tiling windows into it, finding spaces,
focusing and switching them, creating, destroying, moving and swapping them through the
scripting addition, moving windows onto a space, and what SkyLight reports about a space.

## Notes

- Everything here runs on the event-loop thread and takes the managers it touches as explicit
  parameters (decision 13). Views live in the space manager's table keyed by space id and are
  looked up at each use (decision 14); a view is created on first lookup.
- When views are re-keyed (two spaces swapped across displays, a display coming back), every
  handle outside the table that names a view by space id (the managed-window table, the
  insert-feedback table, the drag state's feedback node) is re-pointed in the same step, so no
  handle is ever left naming the wrong view.
- Mission-control indices count from 1 across every display in SkyLight's order; 0 means not
  found (decision 32). They are what the CLI selects spaces by and what queries print
  (decision 3).
- Space operations refuse while Mission Control is active or the display is animating, and
  report why through the space operation outcome.
- Moving windows onto a space tries SkyLight's bridged operation when it resolved at start-up
  (decision 18), then the managed-space move, then the scripting addition, and last the
  compat-id workaround; which path runs depends on the macOS version and is behaviour.
- When the scripting addition cannot focus a space, focus falls back to synthesized Dock swipe
  gestures; their event field numbers and values are what the Dock reads and must not change.
