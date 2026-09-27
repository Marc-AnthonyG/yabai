# window

A window as yabai tracks it: the struct with its flags and liveness cell and what AX and SkyLight
report about it, the AX notifications registered for it, the JSON a window query prints, and the
animation data that carries windows to their new frames.

## Notes

- The liveness cell is shared between a window and its AX refcon (decision 21). Claiming a
  window for destruction is a compare-exchange from alive to dead; every other probe is a load of
  the same cell. An event for a window that is dead or no longer tracked is dropped.
- AX notification callbacks run on the main thread and never touch event-loop-owned memory
  (decision 20). The refcon is a raw `Arc` pointer to the liveness cell; it is released on the
  main queue after the notifications are removed, never from the event-loop thread.
- The animation context is an `Arc` shared with the CVDisplayLink callback thread (decisions 24,
  46). The proxies' target and frame atomics hold `f32` and `f64` bits, not integers, and the
  animations table is locked for exactly the C lock scope.
- The sub-level query sends a packed Mach message whose size and field offsets are asserted at
  compile time (decision 35); its message id differs on Tahoe.
- The window JSON, field order and separators included, is part of the query wire format
  (decisions 3, 29).
