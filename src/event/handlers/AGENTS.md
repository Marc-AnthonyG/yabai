# event/handlers

What the daemon does when each event arrives, one file per subject: applications launching,
terminating, switching to the front, hiding and unhiding; windows appearing, disappearing,
gaining focus, moving, resizing, minimizing and changing title; spaces and displays changing;
mouse clicks, drags and moves; Mission Control; menus opening and closing; system-wide changes
(the Dock restarting or changing preferences, the menu bar hiding, the machine waking); and client
messages.

## Notes

- Everything here runs on the event-loop thread and takes the managers it touches as explicit
  parameters (decision 13). Windows, applications, spaces and displays arrive as ids and are
  looked up again (decision 14).
- A window event is dropped when the window's liveness cell says it is dead or the window is no
  longer tracked (decision 21); a destroy event is acted on only by whoever wins the claim.
- The timing thresholds are observable behaviour computed in `f32` exactly as C did (decisions 3,
  30): the 0.1 s retry of an application that is not ready yet and of the Mission Control exit
  check, the 1.5 s cmd-tab window and the 1.25 s swipe-gesture window, compared against the
  statics the main-thread callbacks stamp.
- Which user signals a handler emits, and in which order relative to its other effects, is
  behaviour (decision 3).
