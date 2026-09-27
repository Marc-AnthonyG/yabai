# Deviations — `w2-display-part1` (W2-display, bodies of `src/display.rs`)

`src/display.c:22` | `TIME_FUNCTION` opened `display_serialize` and expanded to the `PROFILE >= 2` timer anchor | not translated (`DECISIONS.md` 5: the `PROFILE` machinery in `misc/timer.h` is dead code)

`src/display.c:4-5,18` | `#pragma clang diagnostic push` / `ignored "-Wunused-parameter"` / `pop` wrapped `display_handler` so its unused `context` parameter did not warn | no equivalent is written; `context` is named and left unused, and `src/main.rs:2` already carries `#![allow(unused_variables)]`

`src/display.c:79` | `space_manager_mission_control_index(space_list[0])` read element 0 before the `count` loop bound was consulted, so a display whose `Spaces` array was empty read one `uint64_t` past the end of the zero-length `ts_alloc_list` block (the value was then unused, because the loop body never ran) | the read is `space_list.first()`, so an empty list reads nothing and `space_manager_mission_control_index` is not called (`DECISIONS.md` 4)

`src/display.c:202-203,206` | `CFArrayGetValueAtIndex` and `CFDictionaryGetValue` results were handed to `CFEqual` and `CFArrayGetCount` unchecked, so a NULL array element or a missing `"Display Identifier"` / `"Spaces"` key dereferenced NULL | `cfarray_borrow_value_at_index` and `cfdictionary_borrow_value` return `Option`, and a `None` `continue`s to the next entry of the managed-display array (`DECISIONS.md` 4)

`src/display.c:241-243` | inside the inner loop, `CFArrayGetValueAtIndex` and `CFDictionaryGetValue(space_ref, CFSTR("id64"))` were handed to `CFNumberGetType`/`CFNumberGetValue` unchecked, so a NULL space entry or a space dictionary without an `id64` key dereferenced NULL | both borrows are `Option` and a `None` `continue`s, which leaves that space out of the returned list instead of storing an uninitialised `uint64_t` (`DECISIONS.md` 4)

`src/display.c:218-251` | `display_space_list` returned an arena pointer plus an `int *count` out-parameter that was written only on the matching-display path, so the caller read its own initialiser when no display matched | the length collapses into the returned `Option<Vec<SpaceId>>`: `None` is C's `NULL` return, `Some` carries the count as `Vec::len`, and the callers' `if (space_list)` guard becomes the `Option` match (`state-access/display.md` §1, `DECISIONS.md` 17)
