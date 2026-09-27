# Deviations — `w2-space_manager-part1` (`src/space_manager.c:1-620`, `src/space_manager.h`)

One line each: C location | what C did | what Rust does.

`src/space_manager.c:9` (`compare_view`) | defined the `table_compare_func` handed to `table_init` | not translated: `Table<K, V>` compares keys with `K: PartialEq` (`DECISIONS.md` 5, `state-access/space.md` "Not translated").

`src/space_manager.h:55` (`space_manager_mark_view_dirty`) | declared with no definition and no caller | not translated (`DECISIONS.md` 5).

`src/space_manager.c:16`, `:28`, `:49`, `:70` (`TIME_FUNCTION`) | expanded the `PROFILE` timer anchor into the four query functions | not translated; `misc/timer.h`'s profiling machinery is dead code (`DECISIONS.md` 5, 51).

`src/space_manager.c:32` | `if (!space_list) return false;` after `window_space_list`, which returns `NULL` for both failure and an empty list (`window.c:97-98` leaves `space_list` `NULL` when `*count` is 0) | `window_space_list` returns `Vec<SpaceId>` (code on disk, `DECISIONS.md` 42) and the guard is `space_list.is_empty()`, which fires on exactly the same inputs.

`src/space_manager.c:74` | `if (!display_list) return false;` after `display_manager_active_display_list`, which can never return `NULL` (`display_manager.c:400` returns a `ts_alloc_list` block, non-`NULL` even for zero displays) | the dead guard is not translated, so a run with zero active displays still prints `[` then `\n` and returns `true` as C does (`DECISIONS.md` 3, 5).

`src/space_manager.c:105-109` | `space_manager_find_view` calls `view_create(sid)` and then `table_add`s the returned pointer | `view_create(space_id, display_manager, window_manager, space_manager) -> SpaceId` (code on disk) owns the insertion, so the Rust only calls it when the lookup misses and returns the handle.

`src/space_manager.c:116`, `:125`, `:132`, `:205`, `:216`, `:233`, `:359`, `:382`, `:399`, `:411`, `:423`, `:435`, `:447`, `:470` | dereferenced a `struct view *` the caller or `space_manager_find_view` guaranteed non-`NULL` | the view is resolved out of `SpaceManager::view` by `SpaceId` and a lookup miss is an early return (`DECISIONS.md` 4, 14).

`src/space_manager.c:161`, `:189` | `string_equals(char *, char *)` compared two NUL-terminated strings | the frozen signature passes the label as `&[u8]` and `SpaceLabel::label` is a `String`, so the comparison is `space_label.label.as_bytes() == label`, identical for tokens without an interior NUL.

`src/space_manager.c:174`, `:190` | `free(space_label->label)` before `buf_del` | `Vec::swap_remove` drops the owned `String` (`DECISIONS.md` 17).

`src/space_manager.c:262` | `table_for` walked `sm->view` while the body could `table_add` and rehash it through `window_manager_validate_and_check_for_windows_on_space` | the keys are snapshotted with `Table::keys_in_bucket_order()` and the view is re-resolved each iteration (`patterns/state-and-ownership.md` §4.2 R2); the C order is preserved.

`src/space_manager.c:401`, `:413`, `:425`, `:437`, `:477` | passed `view->root`, a `struct window_node *` field | passes `ROOT_NODE_ID`, the arena's always-root index (`DECISIONS.md` 15).

`src/space_manager.c:451` | `assert(node)` is a release no-op, so a `NULL` node was dereferenced by `window_node_flush` at `:454` | `debug_assert!` (`DECISIONS.md` 33) plus a `None` guard on the visible branch only; the not-visible branch still sets `VIEW_IS_DIRTY` exactly as C does (`DECISIONS.md` 4).

`src/space_manager.c:474` | `node->parent->split` dereferenced `node->parent` under the `window_node_is_intermediate` test | the `Option<NodeId>` parent is matched and a `None` returns early (`DECISIONS.md` 4).

`src/space_manager.c:476` | compared the `uint32_t` field `view->auto_balance` against the `enum window_node_split` constant `SPLIT_NONE` | `auto_balance != WindowNodeSplit::None as u32`, the cast written at the point of the comparison (`DECISIONS.md` 30).

`src/space_manager.c:493+508`, `:523+538`, `:559-560+574`, `:588+603`, `:617+627` | `CFNumberGetValue(ref, CFNumberGetType(ref), &destination)` wrote 4 or 8 bytes into a `uint64_t` declared once outside the loops, so a 4-byte store left the upper half holding the previous iteration's value | `cfnumber_read_u64_widening` zero-initialises its destination at every read (`DECISIONS.md` 4, `patterns/memory-text-and-os-objects.md` §1.7).

`src/space_manager.c:503`, `:533`, `:569`, `:598`, `:623` | `CFDictionaryGetValue(display_ref, CFSTR("Spaces"))` and `CFDictionaryGetValue(space_ref, CFSTR("id64"))` were passed on with no `NULL` check | `.unwrap()`, turning the undefined behaviour into a panic that the `main` hook converts to an abort (`DECISIONS.md` 4, 7).

`src/space_manager.c:622`, `:625` | `CFArrayGetValueAtIndex(..., 0)` with no bounds check on either array | a bounds check that returns `SpaceId(0)` when the array is empty (`DECISIONS.md` 4).
