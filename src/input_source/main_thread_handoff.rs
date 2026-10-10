use std::sync::mpsc::channel;
use std::time::Duration;

use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::input_source::keyboard_input_sources::{
    check_that_the_input_source_can_be_selected, select_the_input_source_with_id,
    select_the_keyboard_input_source_after_the_current_one,
};

const LONGEST_THE_EVENT_LOOP_WAITS_FOR_THE_MAIN_THREAD_TO_CHECK_AN_INPUT_SOURCE: Duration =
    Duration::from_secs(2);

pub(crate) fn select_the_next_keyboard_input_source_on_the_main_thread_without_waiting() {
    dispatch_after_on_main_queue(0, || {
        if let Err(failure) = select_the_keyboard_input_source_after_the_current_one() {
            crate::warn!("yabai: input-source cycle: {failure}\n");
        }
    });
}

pub(crate) fn check_on_the_main_thread_that_the_input_source_can_be_selected(
    input_source_id: &str,
) -> Result<(), String> {
    let (outcome_sender, outcome_receiver) = channel();
    let input_source_id_for_the_main_thread = input_source_id.to_owned();
    dispatch_after_on_main_queue(0, move || {
        let _ = outcome_sender.send(check_that_the_input_source_can_be_selected(
            &input_source_id_for_the_main_thread,
        ));
    });
    outcome_receiver
        .recv_timeout(LONGEST_THE_EVENT_LOOP_WAITS_FOR_THE_MAIN_THREAD_TO_CHECK_AN_INPUT_SOURCE)
        .unwrap_or_else(|_| {
            Err(format!(
                "the main thread did not check input source '{input_source_id}' in time."
            ))
        })
}

pub(crate) fn select_the_input_source_on_the_main_thread_without_waiting(input_source_id: String) {
    dispatch_after_on_main_queue(0, move || {
        if let Err(failure) = select_the_input_source_with_id(&input_source_id) {
            crate::warn!("yabai: input-source select: {failure}\n");
        }
    });
}
