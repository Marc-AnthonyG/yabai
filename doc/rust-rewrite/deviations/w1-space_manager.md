# Deviations — `w1-space_manager` (W1-space_manager)

`src/space_manager.c:4` | `TABLE_HASH_FUNC(hash_view)` expanded to `uint64_t hash_view(void *key)` and was installed on `sm->view` by `table_init` | the function is named `hash_view_key` and typed `fn hash_view_key(key: &SpaceId) -> u64`, because `Table::new` takes `hash: fn(&K) -> u64` and `K` is `SpaceId` for `SpaceManager::view` (`state-access/space.md` judgement 10); decision 37's "keep the C name" gives way to the one spelling that compiles at the `Table::new` call site
`src/space_manager.c:9` | `TABLE_COMPARE_FUNC(compare_view)`, the `table_compare_func` installed on `sm->view` | not translated; `Table<K, V>` compares keys with `K: PartialEq` (decision 16), so there is no field for it to live in
`src/space_manager.c:1074` | `space_manager_assign_process_to_space`, defined here and declared at `src/space_manager.h:101`, called from nowhere in `src/**` | not translated (decision 5)
`src/space_manager.c:1079` | `space_manager_assign_process_to_all_spaces`, defined here and declared at `src/space_manager.h:102`, called from nowhere in `src/**` | not translated (decision 5)
`src/space_manager.c:1084` | `space_manager_is_window_on_active_space`, defined here and declared at `src/space_manager.h:103`, called from nowhere in `src/**` | not translated (decision 5)
`src/space_manager.h:55` | `space_manager_mark_view_dirty` is declared, with no definition anywhere in `src/**` and no caller | not translated (decision 5, declarations with no definition or no caller)
