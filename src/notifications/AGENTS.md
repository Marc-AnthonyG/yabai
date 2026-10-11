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
  keyboard tap, the tap that disarms a lone modifier and the observer that switches it run on
  their own `yabai-keyboard-tap` thread with its own run loop, so a main thread waiting on an
  application's AX answer never holds up typing. That thread raises itself
  to the user-interactive quality of service, since every key waits on it, while the rest of the
  daemon stays at the default priority.
- The keyboard tap callback stays short: it reads the hotkey table, swallows the key down of a
  hotkey, hands a shell action to the hotkey spawner thread, so shell hotkeys keep working while
  the event loop is busy and the next key never waits for a process to start, and posts a typed
  action to the event loop. A repeated key down of a shell hotkey is swallowed without running
  it again. It lets everything this process posted through untouched, and never swallows a
  flags change. Failing to start it is a line on stderr and the window manager keeps running
  without hotkeys.
- The tap thread records in the tap's state whether the tap is still starting, running or
  failed to start. The config file's `hotkey add` lines can arrive before the thread has created
  the tap, so only a failure makes `hotkey add` fail; the hotkey is stored either way.
- Every tap sits where hardware events enter, at the HID level, head inserted. WindowServer
  handles symbolic hotkeys such as cmd - tab and cmd - ` before a session-level tap sees them,
  so a keyboard tap there never saw their key down, and releasing the right command key after
  cmd - tab looked like a lone tap of it.
- The always-on keyboard tap asks for key downs and flags changes only. The mouse downs,
  scrolls and media keys (NX_SYSDEFINED) that cancel a lone modifier tap go to a second active
  tap on the same thread, created disabled and enabled only while a lone modifier is armed, so
  no click or scroll waits on yabai otherwise. Being active and on the same run loop, it sees a
  click before the release that follows it. It never swallows anything; macOS disabling it
  forgets the armed lone modifier, and it stays off until the next one is armed.
- The mouse has two taps on the main run loop. The active one carries only the clicks and
  drags it may swallow or repost for mouse-modifier drags and group headers, and is created
  once at start-up. Pointer moves never wait on yabai: they arrive through a listen-only tap,
  together with the Dock swipe gestures focus follows mouse checks, and that tap is re-created
  with or without pointer moves whenever the focus-follows-mouse mode is set. Drags stay on
  the active tap because a mouse up must see the drag that came before it.
- `CGEventTapEnable` is a synchronous round trip to WindowServer, so the disarming tap is never
  switched from inside a tap callback, where the event being handled would wait for it. A
  before-waiting observer on the tap thread's run loop switches it once the callbacks have
  answered their events, and only when whether a lone modifier is armed has changed since it
  last did.
- Under secure input, taps stop receiving key downs but still see flags changes, so a flags
  change while SkyLight reports secure input disarms the lone modifier detector: a modifier
  pressed then is never armed, and one released then is never a tap. That flag lives in shared
  memory SkyLight maps on its first read without a lock; the first read happens on the main
  thread before the tap thread starts.
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
