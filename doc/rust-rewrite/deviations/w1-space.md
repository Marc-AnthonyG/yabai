# Deviations — `w1-space` (W1-space, `src/space.h` + `src/space.c` → `src/space.rs`)

`src/space.c:98` | `space_is_system` was defined and declared (`src/space.h:8`) but called from nowhere in `src/` | not translated (`DECISIONS.md` 5); `src/space.rs` declares six functions where `TRANSLATION_PLAN.md` §3.3 sizes the unit at seven

`src/space.c:17` | `space_window_list_for_connection` returned a `ts_alloc_list` buffer plus an `int *count` out-parameter, and on the `err` path (`SLSCopyWindowsWithOptionsAndTags` returning NULL) returned NULL with `*count` left uninitialised | returns `Vec<WindowId>`; the count is the vector's length, so the NULL-plus-uninitialised-count path becomes an empty vector (`DECISIONS.md` 4, 17, 32)
