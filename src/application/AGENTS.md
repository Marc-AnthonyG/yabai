# application

An application as yabai tracks it: its AX element, connection, process serial number, hidden
state and whether its AX observer is registered, and what AX reports about its focused window and
window list.

## Notes

- Applications are owned by the window manager's table and named by process id (decision 14).
  Everything here runs on the event-loop thread, or on the main thread at start-up before the
  event loop exists.
- An application owns a retained AX element, released when the application is dropped. Its
  observer is not released on drop: unobserving hands it to the main queue, which removes the
  notifications and releases it there (decision 20).
