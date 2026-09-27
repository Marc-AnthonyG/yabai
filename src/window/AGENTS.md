# window

Windows as yabai tracks and drives them: the window struct and what AX and SkyLight report about
it, the window manager's tables and settings, discovering windows and applying rules to them, the
lookups that select a window, and every window command (focus, frame, opacity, layer, shadow,
fullscreen and zoom, floating, placement in a tree, on a grid or on another space, the
scratchpad), including the animation that carries windows to their new frames through proxy
windows.

## Notes

- Apart from the callbacks and threads named below, everything here runs on the event-loop
  thread and takes the managers it touches as explicit parameters (decision 13). Windows and
  applications are named by id and looked up at each use (decision 14); a lookup miss is an early
  return.
- The liveness cell is shared between a window and its AX refcon (decision 21). Claiming a
  window for destruction is a compare-exchange from alive to dead; every other probe is a load of
  the same cell. An event for a window that is dead or no longer tracked is dropped.
- AX notification callbacks run on the main thread and never touch event-loop-owned memory
  (decision 20). The refcon is a raw `Arc` pointer to the liveness cell; it is released on the
  main queue after the notifications are removed, never from the event-loop thread.
- The animation context is an `Arc` shared with the CVDisplayLink callback thread (decisions 24,
  46), and proxy images are captured on scoped builder threads. The proxies' target and frame
  atomics hold `f32` and `f64` bits, not integers, and the animations table is locked for exactly
  the C lock scope.
- Rule matching and reapplication take the rules they work on out of the window manager and put
  them back afterwards; code reached in between sees an empty rule list, or a default rule in the
  slot of the rule being reapplied.
- Bytes other programs read are fixed: the synthesized focus event records and their offsets,
  the sub-level Mach message (decision 35, its message id differs on Tahoe) and the JankyBorders
  notification, whose size is asserted at compile time and whose service name and event numbers
  JankyBorders listens for (decision 3).
- The sleeps are timing behaviour and stay (decision 3): 40 ms between the two focus events,
  100 ms per spin while a native-fullscreen transition finishes, 20 ms after a JankyBorders
  notification that waits.
