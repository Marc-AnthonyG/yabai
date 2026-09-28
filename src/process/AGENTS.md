# process

Processes: the process record shared between threads, the process table and the process
manager's front-process bookkeeping, the `NSRunningApplication` behind a process (its activation
policy and whether it finished launching), and the space a process's first window is on.

## Notes

- A process is an `Arc` whose `terminated`, `ns_application` and `policy` are atomics. The table sits behind a `Mutex` that is never held across an ObjC or AX
  call, because the main-thread process callback updates it too.
- Everything here runs on the event-loop thread or at start-up. A process is observable only
  while its activation policy is regular; one without an `NSRunningApplication` is recorded as
  prohibited.
- XPC services, the fixed name blacklist and processes running under a debugger are ignored.
- The Finder's process serial number is cached at start-up; the Finder is brought to the front
  when a command moves the focused window away and no other window is left to focus.
