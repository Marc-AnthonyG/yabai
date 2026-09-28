use clap::{Args, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::command::selectors::SpaceSelector;
use crate::command::values::{
    ExternalBarPadding, OnOrOff, PackedArgbColor, parse_finite_non_negative_duration_in_seconds,
    parse_opacity_above_zero_up_to_one, parse_opacity_from_zero_to_one,
    parse_packed_argb_color_other_than_zero, parse_positive_font_size_in_points,
    parse_split_ratio_from_one_tenth_to_nine_tenths,
};
use crate::display::manager::DisplayArrangementOrder;
use crate::layout::insertion::WindowInsertionPoint;
use crate::layout::settings::ViewLayout;
use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
use crate::mouse::tap::{MouseMode, MouseModifier};
use crate::support::easing::AnimationEasingType;
use crate::window::manager::{FocusFollowsMouseMode, ShadowRemovalMode, WindowOriginDisplayMode};

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum ConfigCommand {
    /// Change settings; every value is checked before any setting changes
    #[command(arg_required_else_help = true)]
    Set(ConfigSetArguments),
    /// Print every setting as one JSON object, or the bare value of one setting
    Get {
        /// Read the settings as this space sees them: its own values over the global ones
        #[arg(short, long, value_name = "SPACE_SEL")]
        space: Option<SpaceSelector>,
        /// The setting to print, spelled like its flag
        setting: Option<ConfigurationSettingName>,
    },
}

#[derive(Args, Serialize, Deserialize, Debug)]
pub(crate) struct ConfigSetArguments {
    /// Give one space its own values instead of changing the global ones, which the space then
    /// ignores
    #[arg(
        short,
        long,
        value_name = "SPACE_SEL",
        conflicts_with = "SettingsForEverySpace",
        requires = "SettingsASpaceCanOverride"
    )]
    pub(crate) space: Option<SpaceSelector>,
    #[command(flatten)]
    pub(crate) settings_a_space_can_override: SettingsASpaceCanOverride,
    #[command(flatten)]
    pub(crate) settings_for_every_space: SettingsForEverySpace,
}

#[derive(Args, Serialize, Deserialize, Debug, Default)]
pub(crate) struct SettingsASpaceCanOverride {
    /// Tile windows as a binary tree (bsp), pile them up (stack) or leave them be (float)
    #[arg(long)]
    pub(crate) layout: Option<ViewLayout>,
    /// Split along the y-axis (vertical), the x-axis (horizontal) or the longer side (auto)
    #[arg(long)]
    pub(crate) split_type: Option<WindowNodeSplit>,
    /// Padding at the top of the space, in points
    #[arg(long, value_name = "POINTS")]
    pub(crate) top_padding: Option<u32>,
    /// Padding at the bottom of the space, in points
    #[arg(long, value_name = "POINTS")]
    pub(crate) bottom_padding: Option<u32>,
    /// Padding at the left of the space, in points
    #[arg(long, value_name = "POINTS")]
    pub(crate) left_padding: Option<u32>,
    /// Padding at the right of the space, in points
    #[arg(long, value_name = "POINTS")]
    pub(crate) right_padding: Option<u32>,
    /// Gap between two tiled windows, in points
    #[arg(long, value_name = "POINTS")]
    pub(crate) window_gap: Option<u32>,
    /// Balance the tree after every change, along both axes, one axis or none
    #[arg(long)]
    pub(crate) auto_balance: Option<AutoBalanceAxes>,
}

#[derive(Args, Serialize, Deserialize, Debug, Default)]
pub(crate) struct SettingsForEverySpace {
    /// Print debug information to stdout
    #[arg(long)]
    pub(crate) debug_output: Option<OnOrOff>,
    /// Run the config file again whenever it changes; label rules and signals so that a reload
    /// replaces them instead of adding them again
    #[arg(long)]
    pub(crate) reload_config_file_on_change: Option<OnOrOff>,
    /// Keep room for a status bar on the main display, on every display, or nowhere
    #[arg(long, value_name = "MODE:TOP:BOTTOM")]
    pub(crate) external_bar: Option<ExternalBarPadding>,
    /// Opacity of the menu bar, from 0 to 1; at 0 it no longer takes clicks
    #[arg(long, value_name = "OPACITY", value_parser = parse_opacity_from_zero_to_one)]
    pub(crate) menubar_opacity: Option<f32>,
    /// Put the cursor at the centre of a window yabai focuses
    #[arg(long)]
    pub(crate) mouse_follows_focus: Option<OnOrOff>,
    /// Focus the window under the cursor, raising it (autoraise) or not (autofocus)
    #[arg(long)]
    pub(crate) focus_follows_mouse: Option<FocusFollowsMouseMode>,
    /// Order display indexes as macOS does, by x first (horizontal) or by y first (vertical)
    #[arg(long)]
    pub(crate) display_arrangement_order: Option<DisplayArrangementOrder>,
    /// Manage a new window on the display it opened on, the focused display, or the one under
    /// the cursor
    #[arg(long)]
    pub(crate) window_origin_display: Option<WindowOriginDisplayMode>,
    /// Which child a new window becomes when it splits a window
    #[arg(long)]
    pub(crate) window_placement: Option<WindowNodeChild>,
    /// Which window a new window splits
    #[arg(long)]
    pub(crate) window_insertion_point: Option<WindowInsertionPoint>,
    /// Keep windows zoomed through layout changes
    #[arg(long)]
    pub(crate) window_zoom_persist: Option<OnOrOff>,
    /// Skip the space animation when a window on another space gets focus; only without the
    /// scripting addition, which does it better
    #[arg(long)]
    pub(crate) skip_window_focus_animation: Option<OnOrOff>,
    /// Draw window shadows, none, or only on floating windows (needs the scripting addition)
    #[arg(long)]
    pub(crate) window_shadow: Option<ShadowRemovalMode>,
    /// Give focused and unfocused windows their own opacity (needs the scripting addition)
    #[arg(long)]
    pub(crate) window_opacity: Option<OnOrOff>,
    /// Seconds an opacity change takes
    #[arg(long, value_name = "SECONDS", value_parser = parse_finite_non_negative_duration_in_seconds)]
    pub(crate) window_opacity_duration: Option<f32>,
    /// Opacity of the focused window, above 0 up to 1
    #[arg(long, value_name = "OPACITY", value_parser = parse_opacity_above_zero_up_to_one)]
    pub(crate) active_window_opacity: Option<f32>,
    /// Opacity of unfocused windows, above 0 up to 1
    #[arg(long, value_name = "OPACITY", value_parser = parse_opacity_above_zero_up_to_one)]
    pub(crate) normal_window_opacity: Option<f32>,
    /// Seconds a window takes to move into place; above 0 needs the scripting addition and
    /// Screen Recording
    #[arg(long, value_name = "SECONDS", value_parser = parse_finite_non_negative_duration_in_seconds)]
    pub(crate) window_animation_duration: Option<f32>,
    /// Easing curve of window animations (see https://easings.net)
    #[arg(long)]
    pub(crate) window_animation_easing: Option<AnimationEasingType>,
    /// Colour of the insertion preview, 0xAARRGGBB other than 0; until it is set, the accent
    /// colour
    #[arg(long, value_name = "0xAARRGGBB", value_parser = parse_packed_argb_color_other_than_zero)]
    pub(crate) insert_feedback_color: Option<PackedArgbColor>,
    /// Height of the tab header of a window group, in points; 0 draws none
    #[arg(long, value_name = "POINTS")]
    pub(crate) group_header_height: Option<u32>,
    /// Colour of the bar behind the tabs of a group; 0x00000000 draws none
    #[arg(long, value_name = "0xAARRGGBB")]
    pub(crate) group_header_background_color: Option<PackedArgbColor>,
    /// Colour of the tab of the window in front of its group
    #[arg(long, value_name = "0xAARRGGBB", value_parser = parse_packed_argb_color_other_than_zero)]
    pub(crate) group_header_active_color: Option<PackedArgbColor>,
    /// Colour of the tabs of the other windows of a group
    #[arg(long, value_name = "0xAARRGGBB", value_parser = parse_packed_argb_color_other_than_zero)]
    pub(crate) group_header_inactive_color: Option<PackedArgbColor>,
    /// Colour of the title in the tab of the window in front of its group
    #[arg(long, value_name = "0xAARRGGBB", value_parser = parse_packed_argb_color_other_than_zero)]
    pub(crate) group_header_active_text_color: Option<PackedArgbColor>,
    /// Colour of the titles in the tabs of the other windows of a group
    #[arg(long, value_name = "0xAARRGGBB", value_parser = parse_packed_argb_color_other_than_zero)]
    pub(crate) group_header_inactive_text_color: Option<PackedArgbColor>,
    /// Font family of the tab titles, as Font Book lists it
    #[arg(long, value_name = "FAMILY")]
    pub(crate) group_header_font_family: Option<String>,
    /// Font style of the tab titles, such as Bold; empty for the regular style
    #[arg(long, value_name = "STYLE")]
    pub(crate) group_header_font_style: Option<String>,
    /// Size of the tab titles, in points
    #[arg(long, value_name = "POINTS", value_parser = parse_positive_font_size_in_points)]
    pub(crate) group_header_font_size: Option<f32>,
    /// Share of the area a split gives its first child, from 0.1 to 0.9
    #[arg(long, value_name = "RATIO", value_parser = parse_split_ratio_from_one_tenth_to_nine_tenths)]
    pub(crate) split_ratio: Option<f32>,
    /// Key held to move or resize windows with the mouse
    #[arg(long)]
    pub(crate) mouse_modifier: Option<MouseModifierKey>,
    /// What the left button does while the modifier is held
    #[arg(long)]
    pub(crate) mouse_action1: Option<MouseButtonAction>,
    /// What the right button does while the modifier is held
    #[arg(long)]
    pub(crate) mouse_action2: Option<MouseButtonAction>,
    /// What dropping a tiled window on the centre of another does
    #[arg(long)]
    pub(crate) mouse_drop_action: Option<MouseDropAction>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConfigurationSettingName {
    Layout,
    SplitType,
    TopPadding,
    BottomPadding,
    LeftPadding,
    RightPadding,
    WindowGap,
    AutoBalance,
    DebugOutput,
    ReloadConfigFileOnChange,
    ExternalBar,
    MenubarOpacity,
    MouseFollowsFocus,
    FocusFollowsMouse,
    DisplayArrangementOrder,
    WindowOriginDisplay,
    WindowPlacement,
    WindowInsertionPoint,
    WindowZoomPersist,
    SkipWindowFocusAnimation,
    WindowShadow,
    WindowOpacity,
    WindowOpacityDuration,
    ActiveWindowOpacity,
    NormalWindowOpacity,
    WindowAnimationDuration,
    WindowAnimationEasing,
    InsertFeedbackColor,
    GroupHeaderHeight,
    GroupHeaderBackgroundColor,
    GroupHeaderActiveColor,
    GroupHeaderInactiveColor,
    GroupHeaderActiveTextColor,
    GroupHeaderInactiveTextColor,
    GroupHeaderFontFamily,
    GroupHeaderFontStyle,
    GroupHeaderFontSize,
    SplitRatio,
    MouseModifier,
    MouseAction1,
    MouseAction2,
    MouseDropAction,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AutoBalanceAxes {
    Off,
    On,
    XAxis,
    YAxis,
}

impl AutoBalanceAxes {
    pub(crate) fn split_axes_it_balances(self) -> u32 {
        match self {
            AutoBalanceAxes::Off => WindowNodeSplit::None as u32,
            AutoBalanceAxes::On => {
                WindowNodeSplit::Horizontal as u32 | WindowNodeSplit::Vertical as u32
            }
            AutoBalanceAxes::XAxis => WindowNodeSplit::Horizontal as u32,
            AutoBalanceAxes::YAxis => WindowNodeSplit::Vertical as u32,
        }
    }

    pub(crate) fn balancing_split_axes(split_axes: u32) -> AutoBalanceAxes {
        let balances_the_x_axis = split_axes & WindowNodeSplit::Horizontal as u32 != 0;
        let balances_the_y_axis = split_axes & WindowNodeSplit::Vertical as u32 != 0;
        match (balances_the_x_axis, balances_the_y_axis) {
            (true, true) => AutoBalanceAxes::On,
            (true, false) => AutoBalanceAxes::XAxis,
            (false, true) => AutoBalanceAxes::YAxis,
            (false, false) => AutoBalanceAxes::Off,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MouseModifierKey {
    Cmd,
    Alt,
    Shift,
    Ctrl,
    #[value(name = "fn")]
    #[serde(rename = "fn")]
    Function,
}

impl MouseModifierKey {
    pub(crate) fn mouse_modifier(self) -> MouseModifier {
        match self {
            MouseModifierKey::Cmd => MouseModifier::COMMAND,
            MouseModifierKey::Alt => MouseModifier::ALT,
            MouseModifierKey::Shift => MouseModifier::SHIFT,
            MouseModifierKey::Ctrl => MouseModifier::CONTROL,
            MouseModifierKey::Function => MouseModifier::FUNCTION,
        }
    }

    pub(crate) fn of_mouse_modifier(mouse_modifier: MouseModifier) -> Option<MouseModifierKey> {
        match mouse_modifier {
            MouseModifier::COMMAND => Some(MouseModifierKey::Cmd),
            MouseModifier::ALT => Some(MouseModifierKey::Alt),
            MouseModifier::SHIFT => Some(MouseModifierKey::Shift),
            MouseModifier::CONTROL => Some(MouseModifierKey::Ctrl),
            MouseModifier::FUNCTION => Some(MouseModifierKey::Function),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MouseButtonAction {
    Move,
    Resize,
}

impl MouseButtonAction {
    pub(crate) fn mouse_mode(self) -> MouseMode {
        match self {
            MouseButtonAction::Move => MouseMode::Move,
            MouseButtonAction::Resize => MouseMode::Resize,
        }
    }

    pub(crate) fn of_mouse_mode(mouse_mode: MouseMode) -> Option<MouseButtonAction> {
        match mouse_mode {
            MouseMode::Move => Some(MouseButtonAction::Move),
            MouseMode::Resize => Some(MouseButtonAction::Resize),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MouseDropAction {
    Swap,
    Stack,
}

impl MouseDropAction {
    pub(crate) fn mouse_mode(self) -> MouseMode {
        match self {
            MouseDropAction::Swap => MouseMode::Swap,
            MouseDropAction::Stack => MouseMode::Stack,
        }
    }

    pub(crate) fn of_mouse_mode(mouse_mode: MouseMode) -> Option<MouseDropAction> {
        match mouse_mode {
            MouseMode::Swap => Some(MouseDropAction::Swap),
            MouseMode::Stack => Some(MouseDropAction::Stack),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser, ValueEnum};

    use super::{
        AutoBalanceAxes, ConfigCommand, ConfigSetArguments, ConfigurationSettingName,
        MouseButtonAction, MouseDropAction, MouseModifierKey,
    };
    use crate::command::selectors::SpaceSelector;
    use crate::command::values::{
        OnOrOff, PackedArgbColor,
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json,
    };
    use crate::command::{CommandLine, DaemonCommand, TopLevelCommand};
    use crate::display::manager::{DisplayArrangementOrder, ExternalBarMode};
    use crate::layout::insertion::WindowInsertionPoint;
    use crate::layout::settings::ViewLayout;
    use crate::layout::tree::{WindowNodeChild, WindowNodeSplit};
    use crate::support::easing::AnimationEasingType;
    use crate::window::manager::{
        FocusFollowsMouseMode, ShadowRemovalMode, WindowOriginDisplayMode,
    };

    fn parse_config_command(arguments: &[&str]) -> Result<ConfigCommand, clap::Error> {
        let command_line = CommandLine::try_parse_from(
            ["yabai", "config"]
                .into_iter()
                .chain(arguments.iter().copied()),
        )?;
        match command_line.command {
            Some(TopLevelCommand::SentToTheRunningWindowManager(DaemonCommand::Config(
                config_command,
            ))) => Ok(config_command),
            _ => panic!("{arguments:?} should parse as a config command"),
        }
    }

    fn parse_config_set(arguments: &[&str]) -> Result<ConfigSetArguments, clap::Error> {
        match parse_config_command(&[&["set"], arguments].concat())? {
            ConfigCommand::Set(config_set_arguments) => Ok(config_set_arguments),
            ConfigCommand::Get { .. } => panic!("{arguments:?} should parse as config set"),
        }
    }

    #[test]
    fn config_set_takes_several_settings_in_one_call() {
        let arguments = parse_config_set(&[
            "--layout",
            "bsp",
            "--window-placement",
            "second-child",
            "--window-animation-easing",
            "ease-out-cubic",
            "--normal-window-opacity",
            "0.90",
            "--group-header-inactive-color",
            "0x00292e42",
            "--external-bar",
            "all:40:0",
        ])
        .unwrap();

        let global = &arguments.settings_for_every_space;
        assert_eq!(
            arguments.settings_a_space_can_override.layout,
            Some(ViewLayout::BinarySpacePartitioning)
        );
        assert_eq!(global.window_placement, Some(WindowNodeChild::Second));
        assert_eq!(
            global.window_animation_easing,
            Some(AnimationEasingType::EaseOutCubic)
        );
        assert_eq!(global.normal_window_opacity, Some(0.9));
        assert_eq!(
            global.group_header_inactive_color,
            Some(PackedArgbColor(0x00292e42))
        );
        assert_eq!(global.external_bar.unwrap().to_string(), "all:40:0");
    }

    #[test]
    fn one_bad_value_refuses_the_whole_config_set() {
        for arguments in [
            &["--split-ratio", "0.5", "--menubar-opacity", "2"][..],
            &["--debug-output", "on", "--insert-feedback-color", "0x0"],
            &["--layout", "bsp", "--window-placement", "second_child"],
            &["--top-padding", "10", "--bottom-padding", "-1"],
            &["--mouse-modifier", "super"],
        ] {
            assert!(parse_config_set(arguments).is_err(), "{arguments:?}");
        }
    }

    #[test]
    fn a_space_only_takes_the_settings_it_can_override() {
        let arguments = parse_config_set(&["--space", "2", "--bottom-padding", "180"]).unwrap();

        assert_eq!(
            arguments.space,
            Some(SpaceSelector::MissionControlIndex(2.try_into().unwrap()))
        );
        assert_eq!(
            arguments.settings_a_space_can_override.bottom_padding,
            Some(180)
        );
        assert!(parse_config_set(&["--space", "2", "--debug-output", "on"]).is_err());
    }

    #[test]
    fn config_set_without_a_setting_is_refused() {
        assert!(parse_config_set(&[]).is_err());
        assert!(parse_config_set(&["--space", "2"]).is_err());
    }

    #[test]
    fn config_get_takes_a_setting_spelled_like_its_flag_and_an_optional_space() {
        let ConfigCommand::Get { space, setting } =
            parse_config_command(&["get", "--space", "code", "split-type"]).unwrap()
        else {
            panic!("config get should parse as config get");
        };

        assert_eq!(space, Some(SpaceSelector::Label(String::from("code"))));
        assert_eq!(setting, Some(ConfigurationSettingName::SplitType));
        assert!(parse_config_command(&["get", "split_type"]).is_err());
    }

    #[test]
    fn every_setting_name_is_a_flag_of_config_set() {
        let command = CommandLine::command();
        let config_set = command
            .find_subcommand("config")
            .and_then(|config| config.find_subcommand("set"))
            .unwrap();
        let flags_of_config_set: Vec<&str> = config_set
            .get_arguments()
            .filter_map(|argument| argument.get_long())
            .collect();

        for setting in ConfigurationSettingName::value_variants() {
            let name = setting.to_possible_value().unwrap();
            assert!(
                flags_of_config_set.contains(&name.get_name()),
                "{}",
                name.get_name()
            );
        }
        let flags_that_are_not_a_setting = ["space", "help", "version"];
        assert_eq!(
            ConfigurationSettingName::value_variants().len(),
            flags_of_config_set
                .iter()
                .filter(|flag| !flags_that_are_not_a_setting.contains(flag))
                .count()
        );
    }

    #[test]
    fn every_setting_value_is_spelled_alike_on_the_command_line_and_in_json() {
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<ViewLayout>();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<WindowNodeSplit>();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<WindowNodeChild>();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<WindowInsertionPoint>(
        );
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<AutoBalanceAxes>();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<OnOrOff>();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<ExternalBarMode>();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<FocusFollowsMouseMode>(
        );
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<
            DisplayArrangementOrder,
        >();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<
            WindowOriginDisplayMode,
        >();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<ShadowRemovalMode>();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<AnimationEasingType>(
        );
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<MouseModifierKey>();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<MouseButtonAction>();
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<MouseDropAction>();
    }

    #[test]
    fn auto_balance_turns_into_the_split_axes_it_balances_and_back() {
        for axes in AutoBalanceAxes::value_variants() {
            assert_eq!(
                AutoBalanceAxes::balancing_split_axes(axes.split_axes_it_balances()),
                *axes
            );
        }
    }

    #[test]
    fn every_mouse_modifier_key_turns_into_its_modifier_and_back() {
        for key in MouseModifierKey::value_variants() {
            assert_eq!(
                MouseModifierKey::of_mouse_modifier(key.mouse_modifier()),
                Some(*key)
            );
        }
    }
}
