# process

Processes: the process record shared between threads, the process table and the process
manager's front-process bookkeeping, the Carbon handler that reports launches, terminations and
front switches, and the space a process's first window is on.

## Notes

- A process is an `Arc` whose `terminated`, `ns_application` and `policy` are atomics
  (decisions 22, 47). The table sits behind a `Mutex` that is never held across an ObjC or AX
  call (decision 22).
- The Carbon handler runs on the main thread. It creates and removes process records and
  updates the table, then hands the process to the event loop as an event (decision 20);
  everything else here runs on the event-loop thread or at start-up.
- Which applications yabai ever sees is decided here and is observable (decision 3): XPC
  services, the fixed name blacklist and processes running under a debugger are ignored, and a
  launch reported twice for the same process serial number is dropped.
- The Finder's process serial number is cached at start-up; the Finder is brought to the front
  when a command moves the focused window away and no other window is left to focus.
