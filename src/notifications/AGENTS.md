# notifications

How macOS tells the daemon that something happened: registering and removing the AX observers on
applications, on their windows and on the Dock for Mission Control, asking SkyLight for window
notifications, the SkyLight connection notifications, the Carbon process events, the display
reconfiguration callback, the event tap on the mouse, and the NSWorkspace, notification-center
and key-value observers, each with the callback it installs.

## Notes

- Every callback here runs on the main thread, from the main run loop or AppKit's notification
  delivery. The one exception is the key-value observer: registering it with the initial option
  calls it back synchronously on whichever thread registers, the event-loop thread included
  (decision 22).
- No callback dereferences event-loop-owned memory (decision 20). The only shared state a callback
  writes is atomics, the tap's own state and the process table, whose lock is never held across
  an ObjC or AX call (decision 22). Everything else is handed to the event loop as an event.
- A refcon is either an integer id (an application's process id) or a raw `Arc` pointer: a
  window's liveness cell (decision 21) or a process for key-value observing. That `Arc` is
  released on the main queue once the notification is removed, never directly from the
  event-loop thread (decision 20). Removing AX notifications and invalidating their run-loop
  sources is likewise dispatched to the main queue.
- Observing an application succeeds only when all five window notifications register; the two
  menu notifications are optional, and an AX "cannot complete" error marks the application for a
  retry. A notification's bit is its index in its name table (decision 31); the application
  table lives here (decision 50).
- A process launch reported twice for the same process serial number is dropped, and processes
  running under a debugger are ignored (decision 3).
- The exception AppKit throws when removing a key-value observation that it claims is not
  registered is caught and swallowed on purpose (decision 7).
- The SkyLight notification numbers, the event tap mask and the Dock gesture field numbers are
  the platform's values and must not change. The tap re-enables itself when macOS disables it by
  timeout or user input.
