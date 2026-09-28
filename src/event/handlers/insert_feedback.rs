use crate::layout::insertion::{
    advance_the_fade_in_of_every_feedback_window, is_any_feedback_window_still_fading_in,
    schedule_a_fade_in_step_unless_one_is_already_scheduled,
};
use crate::space::manager::SpaceManager;

pub(crate) fn handle_insert_feedback_fade_in_step_event(space_manager: &mut SpaceManager) {
    space_manager.insert_feedback_fade_in_step_is_scheduled = false;
    advance_the_fade_in_of_every_feedback_window(space_manager);

    if is_any_feedback_window_still_fading_in(space_manager) {
        schedule_a_fade_in_step_unless_one_is_already_scheduled(space_manager);
    }
}

#[cfg(test)]
mod tests {
    use super::handle_insert_feedback_fade_in_step_event;
    use crate::space::manager::create_space_manager_without_any_view_with_its_initial_settings;

    #[test]
    fn a_fade_in_step_that_finds_nothing_fading_leaves_no_step_scheduled_so_the_next_overlay_starts_one()
     {
        let mut space_manager = create_space_manager_without_any_view_with_its_initial_settings();
        space_manager.insert_feedback_fade_in_step_is_scheduled = true;

        handle_insert_feedback_fade_in_step_event(&mut space_manager);

        assert!(!space_manager.insert_feedback_fade_in_step_is_scheduled);
    }
}
