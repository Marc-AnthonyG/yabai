# hotkey

Keyboard shortcuts the daemon answers itself: the chord vocabulary (side-agnostic modifiers and
keys named by their ANSI virtual keycode) and the canonical spelling a chord prints in, a hotkey's
action (a yabai command kept as its arguments, or a shell command), and the table of hotkeys the
keyboard tap matches.

## Notes

- The table is a static behind a mutex. The `hotkey` executor changes it on the event-loop
  thread and the keyboard tap reads it on its own thread; neither holds the lock across a call,
  and the tap copies the matched action out before it runs it.
- A new hotkey replaces every hotkey with the same chord and every one with the same non-empty
  label, and takes the place of the first one it replaces, so running the config file again
  keeps the listing order. Removal keeps the order of the others.
- Keys match by virtual keycode, never by character, so a hotkey stays on the same physical key
  whatever layout is selected. Alt, shift, cmd and ctrl are compared exactly, either side
  satisfying each; caps lock is ignored; fn is compared only when the chord names it, because
  the function keys and arrows always carry the fn flag. A chord that names fn wins over the
  same chord without it.
- A typed action is stored as the arguments it was written with and parsed again on every
  press, because `DaemonCommand` is not `Clone`. The client checked that the arguments parse,
  and refused a `hotkey` command among them, when the hotkey was added.
