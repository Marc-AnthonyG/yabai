# Signature changes — `w2c-review-view` (fidelity review of `src/view.rs` and `src/space_manager.rs`)

One line each: C location | current | proposed | why.

`src/space_manager.c:745`, `:801`, `:1011`, `:1150` | `space_manager_swap_space_with_space_on_display(a_display_id, a_space_id, b_display_id, b_space_id, display_manager, window_manager, space_manager)`, `space_manager_swap_space_with_space(acting_space_id, selector_space_id, display_manager, window_manager, space_manager, mission_control_mode)`, `space_manager_switch_space(space_id, display_manager, window_manager, space_manager, mission_control_mode)`, `space_manager_handle_display_add(space_manager, display_id, window_manager)` | append `mouse_drag_state: &mut MouseDragState` to all four | `g_mouse_state.feedback_node` is a `struct window_node *`, so it keeps naming its node when the swap (`:763-774`) or the display add (`:1181-1190`) re-keys the view; the Rust `Option<(SpaceId, NodeId)>` must have its `SpaceId` rewritten at the same point, with the same mapping `space_manager_point_window_manager_view_handles_at_rekeyed_views` already applies to `managed_window` and `insert_feedback`; both callers (`message.rs` `handle_domain_space`, `event_loop.rs` `event_handler_display_added`) already hold a `&mut MouseDragState`

Changed without a request, because nothing outside `src/view.rs` calls it: `view_free_node(space_id, node_id, space_manager, mouse_drag_state)` became `view_free_node(space_id, node_id, window_manager, space_manager, mouse_drag_state)`.

Added, with no C original: `space_manager_point_window_manager_view_handles_at_rekeyed_views(window_manager, space_id_of_view_after_rekeying)` in `src/space_manager.rs`, and `FeedbackWindow::window_id_or_zero` in `src/view.rs`.
