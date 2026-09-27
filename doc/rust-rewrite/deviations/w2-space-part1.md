# Deviations — `w2-space-part1` (W2-space, bodies of `src/space.rs`)

`src/space.c:8-12` | `CFUUIDCreateFromString` was never NULL-checked, so a uuid string SkyLight could not parse passed `NULL` to `CGDisplayGetDisplayIDFromUUID` (tolerated, returns 0) and then to `CFRelease`, which dereferences it | `space_display_id` returns `DisplayId(0)` when `CFUUIDCreateFromString` yields `None`, the value the C reached only by crashing (`DECISIONS.md` 4)

`src/space.c:31-32` | `SLSWindowQueryWindows` and `SLSWindowQueryResultCopyWindows` were never NULL-checked; a `NULL` was dereferenced by `SLSWindowIteratorAdvance` and by `CFRelease` at `src/space.c:74-75` | `space_window_list_for_connection` returns the empty `Vec<WindowId>` — the translation of the C `NULL` return that the `err`/`out` ladder already produces — when either is `None` (`DECISIONS.md` 4)
