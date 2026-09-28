use clap::{ArgGroup, Args, Subcommand};
use serde::{Deserialize, Serialize};

use crate::command::selectors::{DisplaySelector, IndexOrLabelSelector, SpaceSelector};
use crate::command::values::{
    GridPlacement, OnOrOff, parse_opacity_from_zero_to_one, parse_regular_expression_that_compiles,
};
use crate::support::layer::WindowStackingSubLayer;

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum RuleCommand {
    /// Add a rule for the windows that open from now on
    #[command(arg_required_else_help = true)]
    Add {
        /// Remove the rule once it has applied to one window
        #[arg(long)]
        one_shot: bool,
        #[command(flatten)]
        definition: RuleDefinition,
    },
    /// Apply RULE, or the rule the flags define without storing it, or every stored rule but the
    /// one-shot ones, to the windows already open
    Apply {
        /// The stored rule to apply, by index from 0 or by label
        #[arg(value_name = "RULE", conflicts_with = "RuleDefinition")]
        rule: Option<IndexOrLabelSelector>,
        #[command(flatten)]
        definition: RuleDefinition,
    },
    /// Remove a stored rule
    Remove {
        /// The rule to remove, by index from 0 or by label
        #[arg(value_name = "RULE")]
        rule: IndexOrLabelSelector,
    },
    /// Print every stored rule as a JSON array
    List,
}

#[derive(Args, Serialize, Deserialize, Debug, Default)]
#[command(group(
    ArgGroup::new("where_the_window_is_sent")
        .args(["display", "space"])
        .multiple(true)
))]
pub(crate) struct RuleDefinition {
    /// A later rule with the same label replaces this one
    #[arg(long)]
    pub(crate) label: Option<String>,
    /// Match windows whose application name matches REGEX
    #[arg(long, value_name = "REGEX", conflicts_with = "app_not", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) app: Option<String>,
    /// Match windows whose application name does not match REGEX
    #[arg(long, value_name = "REGEX", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) app_not: Option<String>,
    /// Match windows whose title matches REGEX
    #[arg(long, value_name = "REGEX", conflicts_with = "title_not", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) title: Option<String>,
    /// Match windows whose title does not match REGEX
    #[arg(long, value_name = "REGEX", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) title_not: Option<String>,
    /// Match windows whose accessibility role matches REGEX
    #[arg(long, value_name = "REGEX", conflicts_with = "role_not", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) role: Option<String>,
    /// Match windows whose accessibility role does not match REGEX
    #[arg(long, value_name = "REGEX", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) role_not: Option<String>,
    /// Match windows whose accessibility subrole matches REGEX
    #[arg(long, value_name = "REGEX", conflicts_with = "subrole_not", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) subrole: Option<String>,
    /// Match windows whose accessibility subrole does not match REGEX
    #[arg(long, value_name = "REGEX", value_parser = parse_regular_expression_that_compiles)]
    pub(crate) subrole_not: Option<String>,
    /// Send the window to this display
    #[arg(long, value_name = "DISPLAY_SEL")]
    pub(crate) display: Option<DisplaySelector>,
    /// Send the window to this space
    #[arg(long, value_name = "SPACE_SEL")]
    pub(crate) space: Option<SpaceSelector>,
    /// Focus follows the window to the display or space it is sent to
    #[arg(long, requires = "where_the_window_is_sent")]
    pub(crate) follow: bool,
    /// Tile the window (on) or leave it floating (off)
    #[arg(long)]
    pub(crate) manage: Option<OnOrOff>,
    /// Show the window on every space
    #[arg(long)]
    pub(crate) sticky: Option<OnOrOff>,
    /// Put the cursor at the centre of the window when yabai focuses it
    #[arg(long)]
    pub(crate) mouse_follows_focus: Option<OnOrOff>,
    /// Stack the window below, with or above normal windows
    #[arg(long)]
    pub(crate) sub_layer: Option<WindowStackingSubLayer>,
    /// Opacity of the window, from 0 to 1
    #[arg(long, value_name = "OPACITY", value_parser = parse_opacity_from_zero_to_one)]
    pub(crate) opacity: Option<f32>,
    /// Put the window in a native fullscreen space
    #[arg(long)]
    pub(crate) native_fullscreen: Option<OnOrOff>,
    /// Place the floating window on a grid of the display
    #[arg(long, value_name = "ROWS:COLUMNS:X:Y:WIDTH:HEIGHT")]
    pub(crate) grid: Option<GridPlacement>,
    /// Make the window the scratchpad NAME; it floats from then on
    #[arg(long, value_name = "NAME")]
    pub(crate) scratchpad: Option<String>,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{RuleCommand, RuleDefinition};
    use crate::command::selectors::{IndexOrLabelSelector, SpaceSelector};
    use crate::command::values::OnOrOff;
    use crate::command::{CommandLine, DaemonCommand, TopLevelCommand};
    use crate::support::layer::WindowStackingSubLayer;

    fn parse_rule_command(arguments: &[&str]) -> Result<RuleCommand, clap::Error> {
        let command_line = CommandLine::try_parse_from(
            ["yabai", "rule"]
                .into_iter()
                .chain(arguments.iter().copied()),
        )?;
        match command_line.command {
            Some(TopLevelCommand::SentToTheRunningWindowManager(DaemonCommand::Rule(
                rule_command,
            ))) => Ok(rule_command),
            _ => panic!("{arguments:?} should parse as a rule command"),
        }
    }

    fn parse_rule_add(arguments: &[&str]) -> Result<(bool, RuleDefinition), clap::Error> {
        match parse_rule_command(&[&["add"], arguments].concat())? {
            RuleCommand::Add {
                one_shot,
                definition,
            } => Ok((one_shot, definition)),
            _ => panic!("{arguments:?} should parse as rule add"),
        }
    }

    #[test]
    fn a_rule_takes_its_matchers_and_effects_as_flags() {
        let (one_shot, definition) = parse_rule_add(&[
            "--one-shot",
            "--label",
            "Finder",
            "--app",
            "^Finder$",
            "--title-not",
            "(Co(py|nnect)|Move|Info|Pref)",
            "--space",
            "3",
            "--follow",
            "--manage",
            "off",
            "--sub-layer",
            "above",
            "--grid",
            "4:4:1:1:2:2",
        ])
        .unwrap();

        assert!(one_shot);
        assert_eq!(definition.label.as_deref(), Some("Finder"));
        assert_eq!(definition.app.as_deref(), Some("^Finder$"));
        assert!(definition.title.is_none());
        assert!(definition.title_not.is_some());
        assert!(matches!(
            definition.space,
            Some(SpaceSelector::MissionControlIndex(_))
        ));
        assert!(definition.follow);
        assert_eq!(definition.manage, Some(OnOrOff::Off));
        assert_eq!(definition.sub_layer, Some(WindowStackingSubLayer::Above));
        assert_eq!(definition.grid.unwrap().to_string(), "4:4:1:1:2:2");
    }

    #[test]
    fn a_rule_refuses_a_pattern_and_its_negation_together_and_a_broken_pattern() {
        assert!(parse_rule_add(&["--app", "Finder", "--app-not", "Finder"]).is_err());
        assert!(parse_rule_add(&["--title", "(unclosed"]).is_err());
        assert!(parse_rule_add(&["--app", "Finder", "--manage", "yes"]).is_err());
        assert!(parse_rule_add(&["--app", "Finder", "--opacity", "1.5"]).is_err());
        assert!(parse_rule_add(&[]).is_err());
        assert!(parse_rule_add(&["--app", "Finder", "--follow"]).is_err());
        assert!(
            parse_rule_add(&[
                "--app",
                "Finder",
                "--display",
                "2",
                "--space",
                "3",
                "--follow"
            ])
            .is_ok()
        );
    }

    #[test]
    fn rule_apply_takes_a_stored_rule_or_a_definition_but_not_both() {
        assert!(matches!(
            parse_rule_command(&["apply", "Finder"]).unwrap(),
            RuleCommand::Apply {
                rule: Some(IndexOrLabelSelector::Label(_)),
                ..
            }
        ));
        assert!(matches!(
            parse_rule_command(&["apply", "--app", "Finder", "--manage", "off"]).unwrap(),
            RuleCommand::Apply { rule: None, .. }
        ));
        assert!(matches!(
            parse_rule_command(&["apply"]).unwrap(),
            RuleCommand::Apply { rule: None, .. }
        ));
        assert!(parse_rule_command(&["apply", "0", "--app", "Finder"]).is_err());
    }

    #[test]
    fn rule_remove_needs_a_rule() {
        assert!(matches!(
            parse_rule_command(&["remove", "2"]).unwrap(),
            RuleCommand::Remove {
                rule: IndexOrLabelSelector::Index(2)
            }
        ));
        assert!(parse_rule_command(&["remove"]).is_err());
    }
}
