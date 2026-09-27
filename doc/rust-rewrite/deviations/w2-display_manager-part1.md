# Deviations — w2-display_manager-part1 (`src/display_manager.c:1-506`, `src/display_manager.h`)

| C location | what C did | what Rust does |
|---|---|---|
| `src/display_manager.c:7` | `TIME_FUNCTION` profiling anchor in `display_manager_query_displays` | not translated (`DECISIONS.md` 5) |
| `src/display_manager.c:11` | `if (!display_list) return false;` was unreachable because `ts_alloc_list` never returns NULL, so a zero-display list still printed `[` then `\n` and returned `true` | the empty `Vec` returns `false` and writes nothing, as `state-access/display.md` §2 directs for this row; the `[`-without-`]` quirk of `P15` is therefore not reproduced for a zero-display list |
| `src/display_manager.c:80` | `display_manager_main_display_uuid` is defined and declared but never called | not translated (`DECISIONS.md` 5, `state-access/display.md` §3) |
| `src/display_manager.c:99` | `assert(uuid)` vanishes under NDEBUG, so a NULL uuid reached `CFUUIDCreateFromString(NULL)` and then `CFRelease(NULL)`, which crashes | `debug_assert!(uuid.is_some())` followed by an early `DisplayId(0)` |
| `src/display_manager.c:179`, `:209` | a NULL `CFArrayCreateMutableCopy` result would have been passed to `CFArraySortValues` and then indexed | the sort is skipped and the unsorted `SLSCopyManagedDisplays` array is used |
| `src/display_manager.c:202` | `SLSCopyManagedDisplays` is not null-checked, so `CFArrayGetCount(NULL)` dereferences NULL | an early return of `None` |
| `src/display_manager.c:234` | `CGPoint cursor;` is read uninitialised when `SLSGetCurrentCursorLocation` does not write it | initialised to `CGPoint::ZERO` |
| `src/display_manager.c:393` | `uint32_t count;` is read uninitialised when `CGGetActiveDisplayList` does not write it | initialised to `0` |
| `src/display_manager.c:400-402` | the returned buffer always holds `display_manager_active_display_count()` entries while `*count` carries what the second `CGGetActiveDisplayList` probe reported | the `Vec` is truncated to the reported count, so its length is the `*count` every caller loops on |
| `src/display_manager.c:414` | `display_manager_find_element_at_point` returns without releasing `element_ref` when the role comes back NULL — a leak | `CFRetained`'s `Drop` releases it |
| `src/display_manager.c:432-433` | `int element_connection;` and `ProcessSerialNumber element_psn;` are read uninitialised when `SLSGetWindowOwner` / `SLSGetConnectionPSN` fail | both initialised to zero |
| `src/display_manager.c:355` | `display_manager_active_display_is_animating` is defined and declared but never called | not translated (`DECISIONS.md` 5, `state-access/display.md` §3); its trailing comment is carried on the live twin at `:388` |
| `src/display_manager.c:456-458` | `display_uuid` is not null-checked before `SLSSetActiveMenuBarDisplayIdentifier` and `CFRelease` | an early return when `display_uuid` yields `None` |
| `src/display_manager.c:465-467` | `window->application->psn`, `window->id` and `window->ref` are read through pointers that cannot be NULL in a consistent state | the window and its application are looked up by handle (`DECISIONS.md` 14) and a miss returns from the function without focusing, rather than falling into the `else` branch |
