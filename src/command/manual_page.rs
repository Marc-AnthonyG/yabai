use std::io::{self, Write};

use clap::{Arg, Command, CommandFactory};
use clap_mangen::Man;
use clap_mangen::roff::{Roff, roman};

use crate::command::CommandLine;

const MANUAL_PAGE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/doc/yabai.1");
const VARIABLE_THAT_ASKS_TO_WRITE_THE_MANUAL_PAGE: &str = "YABAI_WRITE_MANUAL_PAGE";

fn manual_page_of_the_command_tree() -> String {
    let mut command_line = CommandLine::command().disable_help_subcommand(true);
    command_line.build();
    let manual = Man::new(command_line.clone()).manual("Yabai Manual");

    let mut page = Roff::default().render();
    page.push_str(&roff_without_apostrophe_preambles(|roff| {
        manual.render_title(roff)?;
        manual.render_name_section(roff)?;
        manual.render_synopsis_section(roff)?;
        manual.render_description_section(roff)?;
        manual.render_options_section(roff)
    }));
    page.push_str(".SH COMMANDS\n");
    append_a_subsection_for_every_command_below(&mut page, &command_line);
    page.push_str(&sections_of_the_help_after_the_options(&command_line));
    page
}

fn append_a_subsection_for_every_command_below(page: &mut String, parent: &Command) {
    for command in parent
        .get_subcommands()
        .filter(|command| !command.is_hide_set())
    {
        page.push_str(&format!(
            ".SS \"{}\"\n",
            command.get_bin_name().unwrap_or(command.get_name())
        ));
        page.push_str(&subsection_of_command(command, parent));
        append_a_subsection_for_every_command_below(page, command);
    }
}

fn subsection_of_command(command: &Command, parent: &Command) -> String {
    let command_with_its_own_arguments = command.clone().mut_args(|argument| {
        let is_an_argument_of_the_parent = parent
            .get_arguments()
            .any(|parent_argument| parent_argument.get_id() == argument.get_id());
        argument_hidden_when(argument, is_an_argument_of_the_parent)
    });
    let has_arguments_of_its_own = command_with_its_own_arguments
        .get_arguments()
        .any(|argument| !argument.is_hide_set());
    let synopsis = Man::new(command_with_its_own_arguments.clone());
    let description_and_arguments = Man::new(command_with_its_own_arguments.mut_args(|argument| {
        let says_no_more_than_the_synopsis = argument.is_positional()
            && argument.get_help().is_none()
            && argument.get_possible_values().is_empty();
        argument_hidden_when(argument, says_no_more_than_the_synopsis)
    }));

    let subsection = roff_without_apostrophe_preambles(|roff| {
        if has_arguments_of_its_own {
            synopsis.render_synopsis_section(roff)?;
        }
        description_and_arguments.render_description_section(roff)?;
        description_and_arguments.render_options_section(roff)
    });
    subsection
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            if line.starts_with(".SH ") {
                String::from(".PP\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect()
}

fn argument_hidden_when(argument: Arg, should_hide: bool) -> Arg {
    if should_hide {
        argument.hide(true)
    } else {
        argument
    }
}

fn roff_without_apostrophe_preambles(
    render: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> String {
    let apostrophe_preamble = Roff::default().render();
    let mut roff = Vec::new();
    render(&mut roff).unwrap();
    String::from_utf8(roff)
        .unwrap()
        .replace(&apostrophe_preamble, "")
}

fn sections_of_the_help_after_the_options(command_line: &Command) -> String {
    let help_after_the_options = command_line
        .get_after_long_help()
        .map(ToString::to_string)
        .unwrap_or_default();
    let mut roff = Roff::default();
    for line in help_after_the_options.lines() {
        match line.strip_suffix(':').filter(|_| !line.starts_with(' ')) {
            Some(heading) => {
                roff.control("fi", []);
                roff.control("SH", [heading.to_uppercase().as_str()]);
                roff.control("nf", []);
            }
            None => {
                roff.text([roman(line)]);
            }
        }
    }
    roff.control("fi", []);
    roff.to_roff()
}

#[test]
fn the_manual_page_is_the_one_the_command_tree_generates() {
    let manual_page = manual_page_of_the_command_tree();
    if std::env::var_os(VARIABLE_THAT_ASKS_TO_WRITE_THE_MANUAL_PAGE).is_some() {
        std::fs::write(MANUAL_PAGE_PATH, &manual_page).unwrap();
    }

    assert!(
        std::fs::read_to_string(MANUAL_PAGE_PATH).unwrap() == manual_page,
        "doc/yabai.1 is not what the command tree generates; run `just man`"
    );
}
