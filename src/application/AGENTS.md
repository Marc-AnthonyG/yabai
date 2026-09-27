# application

An application as yabai tracks it: its AX element, connection, process serial number and hidden
state, what AX reports about its focused window and window list, and the AX observer that
reports its windows being created, focused, moved, resized or retitled and its menus opening
and closing.

## Notes

- Applications are owned by the window manager's table and named by process id (decision 14).
  Everything but the observer callback runs on the event-loop thread, or on the main thread
  at start-up before the event loop exists.
- The observer callback runs on the main thread and never touches event-loop-owned memory
  (decision 20). For the application's own notifications its refcon is the process id; for the
  window notifications registered on the same observer it is the window's liveness cell
  (decision 21). Notifications are removed and the run-loop source invalidated on the main
  queue, never from the event-loop thread.
- Observing succeeds only when all five window notifications register; the two menu
  notifications are optional. An AX "cannot complete" error marks the application for a later
  retry. Each notification's bit is its index in the name table (decision 31), and that table
  lives here (decision 50).
