use crate::layout::feedback_window::schedule_the_next_feedback_window_fade_in_step;
use crate::layout::insertion::{
    a_feedback_window_is_still_fading_in, insert_feedback_advance_every_fade_in,
};
use crate::space::manager::SpaceManager;

pub(crate) fn event_handler_insert_feedback_fade_in_step(space_manager: &mut SpaceManager) {
    insert_feedback_advance_every_fade_in(space_manager);

    if a_feedback_window_is_still_fading_in(space_manager) {
        schedule_the_next_feedback_window_fade_in_step();
    }
}
