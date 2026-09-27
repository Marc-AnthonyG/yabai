# Signature changes — `w1-gate`

Rows of `doc/rust-rewrite/state-access/*.md` that the code on disk does not match verbatim, and
why the row is the thing that is wrong. Everything else — 537 of the 540 function rows across the
nine tables — matches character for character after normalising whitespace.

| table row | what the row says | what `src/` has | why |
| --- | --- | --- | --- |
| `window_manager.md:39` `hash_wm` | `fn hash_window_manager_key(key: &u32) -> u64` | `pub(crate) fn hash_wm_window_id(key: &WindowId) -> u64` and `pub(crate) fn hash_wm_process_id(key: &ProcessId) -> u64` in `src/window_manager.rs:125`, `:129` | `DECISIONS.md` 45 splits `hash_wm` into exactly these two functions, keeping the C name, because `Table::new` takes `fn(&K) -> u64` and the seven tables have two key types. The row predates the ruling and also spells a name `DECISIONS.md` 45 does not use. |
| `display.md:105` `display_manager_get_display_for_label` | `fn display_manager_get_display_for_label(display_manager: &mut DisplayManager, label: &[u8]) -> Option<&mut DisplayLabel>` | `pub(crate) fn display_manager_get_display_for_label<'display_manager>(display_manager: &'display_manager mut DisplayManager, label: &[u8]) -> Option<&'display_manager mut DisplayLabel>` in `src/display_manager.rs:69` | The row as written does not compile: with two elided input lifetimes and no `&self`, the output lifetime cannot be elided (E0106). The named lifetime binds the result to `display_manager`, which is what the row intends. |
| `window-application-process.md:207` `process_is_being_debugged` | `fn process_is_being_debugged(process_id: ProcessId) -> bool` | `pub fn process_is_being_debugged(process_id: pid_t) -> bool` in `src/ffi/libsystem.rs:20` | `DECISIONS.md` 42: the code on disk under `src/ffi/` is ground truth for the API it defines. The function is a `sysctl` wrapper that lives beside the `KERN_PROC` constants it needs, it is already written, and `ProcessId` is a newtype over `i32` rather than an alias, so the row's spelling is not the same type. |

## Visibility

`DECISIONS.md` 48 — every function is `pub(crate)` in phase 2 — overrides the `visibility` column
wherever that column says `private`; a document in this folder elaborates the decisions and may
not reopen them. Thirty-three functions were raised to `pub(crate)` to close the gate:
`event_signal_filter`, `event_signal_prepare_commands`, `event_signal_serialize`,
`RuleEffectsFlag::remove`, `hash_view_key`, `space_manager_query_view`,
`space_manager_move_window_list_to_space`, `space_manager_find_first_user_space_for_display`,
`space_manager_is_space_last_user_space`, `space_manager_swap_space_with_space_on_display`,
`window_node_get_child`, `area_make_pair_for_node`, `window_node_is_occupied`,
`window_node_is_right_child`, `balance_node_add`, `window_node_split`, `window_node_destroy`,
`window_node_clear_zoom`, `view_find_min_depth_leaf_node`,
`window_node_collect_subtree_post_order`, `window_display_uuid`, `window_display_space`,
`window_layer`, `window_property_title_ts`, `window_is_minimized`, `window_shadow`,
`window_opacity`, `window_parent`, `SLSGetWindowSubLevel__Internal`, `window_tags`,
`window_subrole`, `window_is_root`, `WorkspaceContext::init`. `PreparedSignalCommand` was raised
with `event_signal_prepare_commands`, which returns it.

Three groups keep the visibility the tables give them because Rust does not allow anything else:
`fn drop` in the eight `impl Drop` blocks and
`userNotificationCenter_shouldPresentNotification` in its trait impl, since a trait impl item may
not carry a visibility modifier; the nine ObjC methods inside
`define_class!(… impl WorkspaceContext …)` in `src/workspace.rs`; and `fn main`, which is the
crate entry point. The private items of `src/ffi/*` and `src/misc/*` are out of scope here — no
row of the nine tables names them, and `DECISIONS.md` 42 makes that code ground truth for the API
it defines.
