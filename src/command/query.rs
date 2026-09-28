use clap::{Args, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::command::selectors::{DisplaySelector, SpaceSelector, WindowSelector};
use crate::command::values::deserialize_a_present_field_as_some;

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum QueryCommand {
    /// Print every display, or the one holding a space or a window, as a JSON array
    Displays {
        #[command(flatten)]
        filter: DisplaysQueryFilter,
        #[command(flatten)]
        field_selection: FieldSelection<DisplayFieldName>,
    },
    /// Print every space, or those of a display or holding a window, as a JSON array
    Spaces {
        #[command(flatten)]
        filter: SpacesQueryFilter,
        #[command(flatten)]
        field_selection: FieldSelection<SpaceFieldName>,
    },
    /// Print every window, or those on a display or a space, as a JSON array
    Windows {
        #[command(flatten)]
        filter: WindowsQueryFilter,
        #[command(flatten)]
        field_selection: FieldSelection<WindowFieldName>,
    },
    /// Print one display as a JSON object
    Display {
        /// The display to print [default: the one showing the active menu bar]
        #[arg(value_name = "DISPLAY_SEL")]
        display: Option<DisplaySelector>,
        #[command(flatten)]
        field_selection: FieldSelection<DisplayFieldName>,
    },
    /// Print one space as a JSON object
    Space {
        /// The space to print [default: the focused space]
        #[arg(value_name = "SPACE_SEL")]
        space: Option<SpaceSelector>,
        #[command(flatten)]
        field_selection: FieldSelection<SpaceFieldName>,
    },
    /// Print one window as a JSON object
    Window {
        /// The window to print [default: the focused window]
        #[arg(value_name = "WINDOW_SEL")]
        window: Option<WindowSelector>,
        #[command(flatten)]
        field_selection: FieldSelection<WindowFieldName>,
    },
}

#[derive(Args, Serialize, Deserialize, Debug)]
pub(crate) struct FieldSelection<FieldName>
where
    FieldName: ValueEnum + Clone + Send + Sync + 'static,
{
    /// Print only these fields [default: every field]
    #[arg(long, value_delimiter = ',', value_name = "FIELD,…")]
    pub(crate) fields: Vec<FieldName>,
}

#[derive(Args, Serialize, Deserialize, Debug)]
#[group(multiple = false)]
pub(crate) struct DisplaysQueryFilter {
    /// Only the display holding this space [default: the focused space]
    #[arg(long, value_name = "SPACE_SEL")]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_a_present_field_as_some"
    )]
    pub(crate) space: Option<Option<SpaceSelector>>,
    /// Only the display holding this window [default: the focused window]
    #[arg(long, value_name = "WINDOW_SEL")]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_a_present_field_as_some"
    )]
    pub(crate) window: Option<Option<WindowSelector>>,
}

#[derive(Args, Serialize, Deserialize, Debug)]
#[group(multiple = false)]
pub(crate) struct SpacesQueryFilter {
    /// Only the spaces of this display [default: the one showing the active menu bar]
    #[arg(long, value_name = "DISPLAY_SEL")]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_a_present_field_as_some"
    )]
    pub(crate) display: Option<Option<DisplaySelector>>,
    /// Only the spaces holding this window [default: the focused window]
    #[arg(long, value_name = "WINDOW_SEL")]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_a_present_field_as_some"
    )]
    pub(crate) window: Option<Option<WindowSelector>>,
}

#[derive(Args, Serialize, Deserialize, Debug)]
#[group(multiple = false)]
pub(crate) struct WindowsQueryFilter {
    /// Only the windows on this display [default: the one showing the active menu bar]
    #[arg(long, value_name = "DISPLAY_SEL")]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_a_present_field_as_some"
    )]
    pub(crate) display: Option<Option<DisplaySelector>>,
    /// Only the windows on this space [default: the focused space]
    #[arg(long, value_name = "SPACE_SEL")]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_a_present_field_as_some"
    )]
    pub(crate) space: Option<Option<SpaceSelector>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[value(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub(crate) enum DisplayFieldName {
    Id,
    Uuid,
    Index,
    Label,
    Frame,
    Spaces,
    HasFocus,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[value(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub(crate) enum SpaceFieldName {
    Id,
    Uuid,
    Index,
    Label,
    Type,
    Display,
    Windows,
    FirstWindow,
    LastWindow,
    HasFocus,
    IsVisible,
    IsNativeFullscreen,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[value(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub(crate) enum WindowFieldName {
    Id,
    Pid,
    App,
    Title,
    Scratchpad,
    Frame,
    Role,
    Subrole,
    RootWindow,
    Display,
    Space,
    Level,
    SubLevel,
    Layer,
    SubLayer,
    Opacity,
    SplitType,
    SplitChild,
    StackIndex,
    CanMove,
    CanResize,
    HasFocus,
    HasShadow,
    HasParentZoom,
    HasFullscreenZoom,
    HasAxReference,
    IsNativeFullscreen,
    IsVisible,
    IsMinimized,
    IsHidden,
    IsFloating,
    IsSticky,
    IsGrabbed,
    IsGrouped,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{QueryCommand, SpaceFieldName, WindowFieldName};
    use crate::command::selectors::WindowSelector;
    use crate::command::values::assert_every_value_is_spelled_alike_on_the_command_line_and_in_json;
    use crate::command::{CommandLine, DaemonCommand, TopLevelCommand};

    fn parse_query_command(arguments: &[&str]) -> Result<QueryCommand, clap::Error> {
        let command_line = CommandLine::try_parse_from(
            ["yabai", "query"]
                .into_iter()
                .chain(arguments.iter().copied()),
        )?;
        match command_line.command {
            Some(TopLevelCommand::SentToTheRunningWindowManager(DaemonCommand::Query(
                query_command,
            ))) => Ok(query_command),
            _ => panic!("{arguments:?} should parse as a query command"),
        }
    }

    #[test]
    fn a_filter_given_without_a_selector_means_the_focused_one() {
        let QueryCommand::Windows { filter, .. } =
            parse_query_command(&["windows", "--space"]).unwrap()
        else {
            panic!("query windows should parse as query windows");
        };

        assert_eq!(filter.space, Some(None));
        assert_eq!(filter.display, None);
    }

    #[test]
    fn a_plural_query_takes_at_most_one_filter() {
        assert!(parse_query_command(&["windows", "--space", "2", "--display", "1"]).is_err());
        assert!(parse_query_command(&["spaces", "--display", "--window"]).is_err());
    }

    #[test]
    fn a_singular_query_takes_its_selector_as_an_argument() {
        let QueryCommand::Window { window, .. } =
            parse_query_command(&["window", "stack.next"]).unwrap()
        else {
            panic!("query window should parse as query window");
        };

        assert!(matches!(window, Some(WindowSelector::InTheStack(_))));
        assert!(parse_query_command(&["space", "0"]).is_err());
    }

    #[test]
    fn fields_are_the_json_keys_of_the_entity_separated_by_commas() {
        let QueryCommand::Windows {
            field_selection, ..
        } = parse_query_command(&["windows", "--fields", "id,app,is_minimized"]).unwrap()
        else {
            panic!("query windows should parse as query windows");
        };

        assert_eq!(
            field_selection.fields,
            [
                WindowFieldName::Id,
                WindowFieldName::App,
                WindowFieldName::IsMinimized
            ]
        );
        assert!(parse_query_command(&["windows", "--fields", "is-minimized"]).is_err());
        assert!(parse_query_command(&["spaces", "--fields", "app"]).is_err());
    }

    #[test]
    fn every_field_name_is_spelled_alike_on_the_command_line_and_in_json() {
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<WindowFieldName>();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<SpaceFieldName>();
    }
}
