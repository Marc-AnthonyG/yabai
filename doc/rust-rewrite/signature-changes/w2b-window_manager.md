# Signature changes — `w2b-window_manager` (W2-window_manager part 2)

`src/window_manager.rs` as published by this unit does **not** compile against the `src/view.rs`,
`src/space.rs` and `src/sa.rs` currently in the repository until the three requests of the first
section are applied. They were applied in this unit's isolated copy only, where both targets check
with zero errors; the exact edits are scripted in
`/private/tmp/claude-501/-Users-marc-anthonygirard-repository-yabai/b3433984-c738-4628-b336-21d60e0ee8ed/scratchpad/w2b/apply_requested_changes_to_other_modules.py`.

## Requests for other modules' files

`src/view.h:57-66` (`src/view.rs`, `struct WindowProxy`) | `pub(crate) struct WindowProxy { id: WindowId, context: *mut CGContext, target_x: AtomicU32, target_y: AtomicU32, target_width: AtomicU32, target_height: AtomicU32, frame: CGRect, level: i32, sub_level: i32, image: Option<*mut CGImage> }` | `pub(crate) struct WindowProxy { id: AtomicU32, core_graphics_objects: Mutex<WindowProxyCoreGraphicsObjects>, target_x: AtomicU32, target_y: AtomicU32, target_width: AtomicU32, target_height: AtomicU32, frame_origin_x: AtomicU64, frame_origin_y: AtomicU64, frame_size_width: AtomicU64, frame_size_height: AtomicU64, level: AtomicI32, sub_level: AtomicI32 }` plus a new `pub(crate) struct WindowProxyCoreGraphicsObjects { context: Option<CFRetained<CGContext>>, image: Option<CFRetained<CGImage>> }`, every field `pub(crate)`; `src/view.rs` gains the imports `AtomicI32` and `CFRetained` | the proxy lives in an `AnimationContext` shared through `Arc` by `window_animations_table`, the builder threads and the display-link refcon, and it is written after it is shared (by the builders, by a superseding batch at `src/window_manager.c:635-663`, by the final tick at `:585`) while a superseded batch's display-link tick reads `id` and `frame`. With plain fields the only way to write them is `&mut` from a shared `Arc` — undefined behaviour, and a data race on `id` besides. This is the shape `THREADS.md` §8.1 specifies (atomics for the raced and written-after-sharing scalars, a mutex for the two CF objects, which are only touched by one thread at a time); it supersedes the plain types of `GLOSSARY.md` §3.13 for these six fields. `unsafe impl Send`/`Sync for AnimationContext` stay: `CGContext` and `CGImage` are not `Send`/`Sync` in `objc2-core-graphics`, and they are only reached under the proxy's mutex

`src/sa.m:553-554`, `:569-570` (`src/sa.rs`, `scripting_addition_swap_window_proxy_in` / `_out`) | `pack(&mut bytes, &mut length, &animation.proxy.id.0.to_ne_bytes())` | `pack(&mut bytes, &mut length, &animation.proxy.id.load(Ordering::Relaxed).to_ne_bytes())` in both functions | follows from the `WindowProxy::id` request; no signature changes

`src/space.c:17-81` (`src/space.rs`, `space_window_list_for_connection`) | `pub(crate) fn space_window_list_for_connection(space_list: &[SpaceId], connection_id: i32, include_minimized: bool, window_manager: &mut WindowManager) -> Vec<WindowId>` | `-> Option<Vec<WindowId>>`, returning `None` exactly where the C returns NULL (`SLSCopyWindowsWithOptionsAndTags` failed, a zero count, and the two query/iterator NULL checks already recorded in `deviations/w2-space-part1.md`) and `Some(window_list)` after the iterator loop even when every window was filtered out; the initial `let mut window_list = Vec::new();` moves to the `Vec::with_capacity` line so no unused assignment is left. `space_window_list` keeps `-> Vec<WindowId>` and appends `.unwrap_or_default()` | `src/window_manager.c:1604` returns that pointer from `window_manager_existing_application_window_list`, and its two callers test it: `:1613` returns before reconciling the application's AX windows when it is NULL, and `:2533` returns before `scripting_addition_order_window_in`. A non-NULL list of zero windows (every SkyLight window filtered out) takes the other path in both, so a `Vec` that merges the two cases changes which windows are created at start-up and on launch. The other C callers (`space_window_list` users in `view.c`, `space_manager.c`, `event_loop.c`, `window_manager.c:914`, `:2631`, `:2688`) treat NULL like an empty list, so `space_window_list` keeps its `Vec`

## Changes to this module's own signatures

`src/window_manager.c:1580` | `pub(crate) fn window_manager_existing_application_window_list(process_id: Option<ProcessId>, window_manager: &mut WindowManager) -> Vec<WindowId>` | `-> Option<Vec<WindowId>>` | the C returns NULL both when no display yields a space list and when SkyLight returns no list, and both callers (`:1613`, `:2533`) return early on NULL; `None` carries that. No caller outside this file

`src/window_manager.c:463` | `pub(crate) fn window_manager_create_window_proxy(animation_connection: i32, alpha: f32, proxy: &mut WindowProxy)` | `proxy: &WindowProxy` | the proxy is reached through the shared `Arc<AnimationContext>`; the writes go through its atomics and mutex (request above)

`src/window_manager.c:489` | `pub(crate) fn window_manager_destroy_window_proxy(animation_connection: i32, proxy: &mut WindowProxy)` | `proxy: &WindowProxy` | same

`src/window_manager.c:507` | `pub(crate) fn window_manager_build_window_proxy_thread_proc(window_animation: &mut WindowAnimation)` | `window_animation: &WindowAnimation` | same; `state-access/window_manager.md` judgement 6 chose `&mut` on the assumption that the batch is mutated before it is shared, but `window_animations_table` already holds clones of the `Arc` when the builders run (`THREADS.md` §8.2)

## Items removed from `src/window_manager.rs`

`fn window_animation_in_batch_at_index(animation_context: &Arc<AnimationContext>, index: usize) -> &mut WindowAnimation` (added by part 1) | removed | it produced `&mut` from a shared `Arc` via `Arc::as_ptr(..).cast_mut()` — concurrently from several builder threads for the same `Vec` header — which is undefined behaviour; every caller now takes `&animation_context.animation_list[index]`

## Items added to `src/window_manager.rs`, private to the module

`src/window_manager.c:468`, `:481`, `:515`, `:560-566`, `:635-638` | — | `fn load_window_proxy_frame(proxy: &WindowProxy) -> CGRect` and `fn store_window_proxy_frame(proxy: &WindowProxy, frame: CGRect)` | read and write the four `AtomicU64` that hold `proxy.frame`

`src/window_manager.c:1060-1136` | — | `fn window_node_stack_of_window_in_active_view(space_manager: &mut SpaceManager, window_manager: &mut WindowManager, window_id: WindowId, display_manager: &mut DisplayManager) -> Option<([WindowId; NODE_MAX_WINDOW_COUNT], [WindowId; NODE_MAX_WINDOW_COUNT], i32)>` | the six `*_window_in_stack` functions open with the same four statements (`space_manager_find_view` of the active space, the `if (!view)` guard, `view_find_window_node`, the `if (!node)` guard); the helper runs them once and hands back a copy of the node's `window_list`, `window_order` and `window_count`, which is all the six bodies read

`src/window_manager.c:1184`, `:1198`, `:1212`, `:1229`, `:1246`, `:1263` | — | `fn window_node_sibling(space_manager: &mut SpaceManager, space_id: SpaceId, node_id: NodeId, parent_node_id: NodeId) -> Option<NodeId>` | the six family finders each evaluate `window_node_is_left_child(node) ? parent->right : parent->left`; the helper evaluates `window_node_is_left_child` first and then reads the parent's other child, the C order

`src/window_manager.c:1187`, `:1201`, `:1215`, `:1232`, `:1249`, `:1266` | — | `fn window_node_first_window_in_order(space_manager: &SpaceManager, space_id: SpaceId, node_id: NodeId) -> Option<WindowId>` | `node->window_order[0]` through the arena, `None` on a lookup miss

`src/window_manager.c:1925`, `:1996-1998`, `:2032-2034`, `:2102`, `:2308`, `:2406`, `:2473`, `:2481`, `:2486` | — | `fn window_manager_focus_window_with_raise_resolving_its_application(window_manager: &WindowManager, window_id: WindowId)` | every C call site spells `window_manager_focus_window_with_raise(&w->application->psn, w->id, w->ref)`; the helper resolves the `Window` and its `Application` by handle and makes exactly that call, doing nothing on a lookup miss
