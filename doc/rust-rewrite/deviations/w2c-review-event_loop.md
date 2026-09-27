# Deviations — `w2c-review-event_loop` (`src/event_loop.rs`, `src/window.rs`)

`src/window.c:127-128`, `:135-136` | `int connection` and `pid_t pid` were left uninitialised when `SLSGetWindowOwner` or `SLSConnectionGetPID` failed, so the garbage went on to `SLSConnectionGetPID`, was printed as `"pid"` and was handed to `proc_name` | both start at `0`

This withdraws the `src/window.c:173-174` entry of `deviations/w2-window-part1.md`. `process_name` is a function-local `static` in C and `proc_name` leaves it untouched on failure, which is defined behaviour: a failing lookup prints the name the previous call stored, and the empty string before any call succeeded. `window_nonax_serialize` keeps that buffer as a function-local `static PROCESS_NAME: Mutex<[u8; PROC_PIDPATHINFO_MAXSIZE]>` again.
