# hotkey

Keyboard shortcuts the daemon answers itself: the chord vocabulary (side-agnostic modifiers, keys
named by their ANSI virtual keycode, lone modifiers told apart by side) and the canonical spelling
a chord prints in, a hotkey's action (a yabai command kept parsed beside its arguments, or a
shell command),
the table of hotkeys the keyboard tap matches, the state the tap keeps to tell a lone
modifier tap from a modifier held for a chord, and the thread that starts shell actions.

## Notes

- The table is a static behind a mutex. The `hotkey` executor changes it on the event-loop
  thread and the keyboard tap reads it on its own thread; neither holds the lock across a call,
  and the tap copies the matched action out before it runs it. The executor holds the lock for
  the change alone, and `hotkey list` copies the hotkeys out and serializes the copy after
  releasing it, so a key down never waits behind a listing.
- A new hotkey replaces every hotkey with the same chord and every one with the same non-empty
  label, and takes the place of the first one it replaces, so running the config file again
  keeps the listing order. Removal keeps the order of the others.
- Shell actions start on their own `yabai-shell-hotkey-spawner` thread, fed by a channel, never
  on the tap thread, which would hold every following key until the process had started, and
  not through the event loop, so they still start while it is busy. The thread runs at the
  user-initiated quality of service: a press waits for its result, but no key waits on it. A yabai command runs again
  on every key repeat; a shell command runs once per press, since holding a key that opens a
  panel would otherwise close it again.
- Keys match by virtual keycode, never by character, so a hotkey stays on the same physical key
  whatever layout is selected. Alt, shift, cmd and ctrl are compared exactly, either side
  satisfying each; caps lock is ignored; fn is compared only when the chord names it, because
  the function keys and arrows always carry the fn flag. A chord that names fn wins over the
  same chord without it.
- A typed action is parsed once, when the hotkey is added, and kept behind an `Arc` beside the
  arguments it was written with: a press clones the `Arc` under the table lock and the event
  loop runs a clone of the parsed command, so no press builds clap's command tree. The
  arguments are what `hotkey list` prints and what crosses the socket; the daemon parses them
  again as it decodes the request.
- The lone modifier detector arms on the flags change of that physical key with the key's
  device-dependent bit set, and only for a lone modifier some hotkey binds: the executor tells
  it which, as a bit set, under the table lock after every change, so a shift pressed while
  typing never arms it. Pressing any other lone modifier still disarms the armed one, so does
  anything else the keyboard tap sees but the momentum scrolling that carries on after the
  fingers left the trackpad, and the release of the same key within a second is a tap. The flags-changed keycodes and device-dependent bits
  are the platform's values and must not change.
- The mouse tap also disarms the detector on every mouse down it handles, from the main thread:
  it swallows mouse-modifier clicks, and once focus-follows-mouse re-creates it after the
  keyboard tap it sits ahead of it, so the keyboard tap may never see that mouse down.
