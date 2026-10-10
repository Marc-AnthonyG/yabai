# notifications

How macOS tells the daemon that something happened: registering and removing the AX observers on
applications, on their windows and on the Dock for Mission Control, asking SkyLight for window
notifications, the SkyLight connection notifications, the Carbon process events, the display
reconfiguration callback, the event taps on the mouse and on the keyboard, and the NSWorkspace,
notification-center and key-value observers, each with the callback it installs. It also reads the
system accent colour in sRGB, at start-up and whenever the colour preferences change, and posts it
to the event loop.

## Notes

- Every callback here runs on the main thread, from the main run loop or AppKit's notification
  delivery, with two exceptions. Registering the key-value observer with the initial option
  calls it back synchronously on whichever thread registers, the event-loop thread included. The
  keyboard tap runs on its own `yabai-keyboard-tap` thread with its own run loop, so a main
  thread waiting on an application's AX answer never holds up typing.
- The keyboard tap callback stays short: it reads the hotkey table, swallows the key down of a
  hotkey, spawns a shell action itself, so shell hotkeys keep working while the event loop is
  busy, and posts a typed action to the event loop. It lets everything this process posted
  through untouched, and never swallows a flags change. Failing to create it is a line on
  stderr; the window manager keeps running without hotkeys.
- Both taps sit where hardware events enter, at the HID level, head inserted. WindowServer
  handles symbolic hotkeys such as cmd - tab and cmd - ` before a session-level tap sees them,
  so a keyboard tap there never saw their key down, and releasing the right command key after
  cmd - tab looked like a lone tap of it.
- Under secure input, taps stop receiving key downs but still see flags changes, so a lone
  modifier tap is not run while SkyLight reports secure input. That flag lives in shared memory
  SkyLight maps on its first read without a lock; the first read happens on the main thread
  before the tap thread starts.
- No callback dereferences event-loop-owned memory. The only shared state a callback
  writes is atomics, the tap's own state and the process table, whose lock is never held across
  an ObjC or AX call. Everything else is handed to the event loop as an event.
- A refcon is either an integer id (an application's process id) or a raw `Arc` pointer: a
  window's liveness cell or a process for key-value observing. That `Arc` is
  released on the main queue once the notification is removed, never directly from the
  event-loop thread. Removing AX notifications and invalidating their run-loop
  sources is likewise dispatched to the main queue.
- Observing an application succeeds only when all five window notifications register; the two
  menu notifications are optional, and an AX "cannot complete" error marks the application for a
  retry. A notification's bit is its index in its name table; the application
  table lives here.
- A process launch reported twice for the same process serial number is dropped, and processes
  running under a debugger are ignored.
- The exception AppKit throws when removing a key-value observation that it claims is not
  registered is caught and swallowed on purpose.
- The accent colour is read on the main thread. The distributed colour-preferences notification
  reads it one main-queue turn later, so AppKit's own observers have refreshed the colour first;
  AppKit's system-colours notification reads it at once.
- The SkyLight notification numbers, the event tap masks and the Dock gesture field numbers are
  the platform's values and must not change. Each tap re-enables itself when macOS disables it by
  timeout or user input; the keyboard tap also forgets an armed lone modifier then.
