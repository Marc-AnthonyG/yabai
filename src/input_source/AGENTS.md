# input_source

Keyboard input sources through Text Input Sources: the keyboard layouts and input modes a user
can switch to, selecting the one after the current one, and checking and selecting one by id.

## Notes

- Text Input Sources is not thread safe and has to run on the main thread, so everything in
  `keyboard_input_sources` runs inside a main-queue block and the executor never calls it
  directly. `cycle`, and the selection `select` makes, are dispatched without waiting and report
  a failure on stderr. `select` first checks the id in a main-queue block the event loop waits
  for, two seconds at most, so an unknown or disabled id fails the command while a busy main
  thread cannot hold the event loop any longer than that.
- Cycling goes over the enabled, select-capable sources whose type is keyboard layout or input
  mode, in the order Text Input Sources lists them; palettes, press-and-hold and input methods
  that only own modes are skipped. When the current source is not among them, the cycle starts
  at the first one.
