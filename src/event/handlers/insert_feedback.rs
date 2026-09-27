use crate::layout::insertion::{
    a_feedback_window_is_still_fading_in, insert_feedback_advance_every_fade_in,
    schedule_a_fade_in_step_unless_one_is_already_scheduled,
};
use crate::space::manager::SpaceManager;

pub(crate) fn event_handler_insert_feedback_fade_in_step(space_manager: &mut SpaceManager) {
    space_manager.insert_feedback_fade_in_step_is_scheduled = false;
    insert_feedback_advance_every_fade_in(space_manager);

    if a_feedback_window_is_still_fading_in(space_manager) {
        schedule_a_fade_in_step_unless_one_is_already_scheduled(space_manager);
    }
}

#[cfg(test)]
mod tests {
    use super::event_handler_insert_feedback_fade_in_step;
    use crate::space::manager::space_manager_without_any_view_with_its_initial_settings;

    #[test]
    fn a_fade_in_step_that_finds_nothing_fading_leaves_no_step_scheduled_so_the_next_overlay_starts_one()
     {
        let mut space_manager = space_manager_without_any_view_with_its_initial_settings();
        space_manager.insert_feedback_fade_in_step_is_scheduled = true;

        event_handler_insert_feedback_fade_in_step(&mut space_manager);

        assert!(!space_manager.insert_feedback_fade_in_step_is_scheduled);
    }
}
