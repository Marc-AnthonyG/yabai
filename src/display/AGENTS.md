# display

Displays: the display manager's settings, display labels, identifying a display (by UUID, the
main, active, cursor or Dock display, the one under a point), the arrangement order and its
neighbours, the bounds left for windows once the menu bar, notch, Dock and external bar are
taken out, the spaces SkyLight assigns each display, and focusing a display.

## Notes

- Everything here runs on the event-loop thread, or on the main thread at start-up, and takes the
  managers it touches as explicit parameters.
- Arrangement indices count from 1; 0 means none. With a non-default order, displays are sorted
  by their centre coordinate on the chosen axis, then on the other.
- In the usable bounds the menu bar rectangle is one point taller than reported, the notch only
  counts when the menu bar is hidden, and the Dock orientation values are CoreDock's. The menu
  bar rectangle comes from a different SkyLight call on each architecture, because the one x86_64
  uses is broken on Apple Silicon.
- Whether a display is animating is only asked of SkyLight on Big Sur to Ventura; later
  versions always report that it is not.
- The arrangement-order and external-bar name tables are CLI spellings indexed by the enum
  discriminant.
