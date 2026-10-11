use crate::command::input_source::InputSourceCommand;
use crate::input_source::main_thread_handoff::{
    check_on_the_main_thread_that_the_input_source_can_be_selected,
    select_the_input_source_on_the_main_thread_without_waiting,
    select_the_next_keyboard_input_source_on_the_main_thread_without_waiting,
};

pub(crate) fn run_input_source_command(command: InputSourceCommand) -> Result<String, String> {
    match command {
        InputSourceCommand::Cycle => {
            select_the_next_keyboard_input_source_on_the_main_thread_without_waiting();
        }
        InputSourceCommand::Select { input_source_id } => {
            check_on_the_main_thread_that_the_input_source_can_be_selected(&input_source_id)?;
            select_the_input_source_on_the_main_thread_without_waiting(input_source_id);
        }
    }
    Ok(String::new())
}
