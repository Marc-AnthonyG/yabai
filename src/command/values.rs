use std::fmt;
use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::display::manager::ExternalBarMode;

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum OnOrOff {
    On,
    Off,
}

impl OnOrOff {
    pub(crate) fn is_on(self) -> bool {
        self == OnOrOff::On
    }

    pub(crate) fn of_switch(is_on: bool) -> OnOrOff {
        if is_on { OnOrOff::On } else { OnOrOff::Off }
    }
}

pub(crate) fn command_line_spelling_of<Value: ValueEnum>(value: &Value) -> String {
    value
        .to_possible_value()
        .map(|possible_value| possible_value.get_name().to_owned())
        .unwrap_or_default()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct PackedArgbColor(pub(crate) u32);

const MOST_HEXADECIMAL_DIGITS_OF_A_PACKED_ARGB_COLOR: usize = 8;

impl FromStr for PackedArgbColor {
    type Err = String;

    fn from_str(text: &str) -> Result<PackedArgbColor, String> {
        let refusal = || format!("'{text}' is not a colour written 0xAARRGGBB");
        let hexadecimal_digits = text
            .strip_prefix("0x")
            .or_else(|| text.strip_prefix("0X"))
            .ok_or_else(refusal)?;
        let has_one_to_eight_hexadecimal_digits = (1
            ..=MOST_HEXADECIMAL_DIGITS_OF_A_PACKED_ARGB_COLOR)
            .contains(&hexadecimal_digits.len())
            && hexadecimal_digits
                .chars()
                .all(|character| character.is_ascii_hexdigit());
        if !has_one_to_eight_hexadecimal_digits {
            return Err(refusal());
        }
        u32::from_str_radix(hexadecimal_digits, 16)
            .map(PackedArgbColor)
            .map_err(|_| refusal())
    }
}

impl fmt::Display for PackedArgbColor {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "0x{:08x}", self.0)
    }
}

impl TryFrom<String> for PackedArgbColor {
    type Error = String;

    fn try_from(text: String) -> Result<PackedArgbColor, String> {
        text.parse()
    }
}

impl From<PackedArgbColor> for String {
    fn from(color: PackedArgbColor) -> String {
        color.to_string()
    }
}

pub(crate) fn parse_packed_argb_color_other_than_zero(
    text: &str,
) -> Result<PackedArgbColor, String> {
    let color: PackedArgbColor = text.parse()?;
    if color.0 == 0 {
        return Err(String::from("the colour cannot be 0x00000000"));
    }
    Ok(color)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct ExternalBarPadding {
    pub(crate) mode: ExternalBarMode,
    pub(crate) top_padding: u32,
    pub(crate) bottom_padding: u32,
}

impl FromStr for ExternalBarPadding {
    type Err = String;

    fn from_str(text: &str) -> Result<ExternalBarPadding, String> {
        let refusal = || format!("'{text}' is not MODE:TOP:BOTTOM, MODE being main, all or off");
        let [mode, top_padding, bottom_padding] = text
            .split(':')
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| refusal())?;
        Ok(ExternalBarPadding {
            mode: ExternalBarMode::from_str(mode, false).map_err(|_| refusal())?,
            top_padding: top_padding.parse().map_err(|_| refusal())?,
            bottom_padding: bottom_padding.parse().map_err(|_| refusal())?,
        })
    }
}

impl fmt::Display for ExternalBarPadding {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            command_line_spelling_of(&self.mode),
            self.top_padding,
            self.bottom_padding
        )
    }
}

impl TryFrom<String> for ExternalBarPadding {
    type Error = String;

    fn try_from(text: String) -> Result<ExternalBarPadding, String> {
        text.parse()
    }
}

impl From<ExternalBarPadding> for String {
    fn from(external_bar_padding: ExternalBarPadding) -> String {
        external_bar_padding.to_string()
    }
}

// serde reads a null as None, so without this a filter given without a selector, serialised as
// null, would come back as no filter at all.
pub(crate) fn deserialize_a_present_field_as_some<'de, Deserializer, Value>(
    deserializer: Deserializer,
) -> Result<Option<Value>, Deserializer::Error>
where
    Deserializer: serde::Deserializer<'de>,
    Value: Deserialize<'de>,
{
    Value::deserialize(deserializer).map(Some)
}

fn parse_finite_number(text: &str) -> Result<f32, String> {
    text.parse::<f32>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("'{text}' is not a number"))
}

pub(crate) fn parse_opacity_from_zero_to_one(text: &str) -> Result<f32, String> {
    let opacity = parse_finite_number(text)?;
    if !(0.0..=1.0).contains(&opacity) {
        return Err(format!("the opacity {opacity} is not from 0 to 1"));
    }
    Ok(opacity)
}

pub(crate) fn parse_opacity_above_zero_up_to_one(text: &str) -> Result<f32, String> {
    let opacity = parse_finite_number(text)?;
    if opacity <= 0.0 || opacity > 1.0 {
        return Err(format!("the opacity {opacity} is not above 0 and up to 1"));
    }
    Ok(opacity)
}

pub(crate) fn parse_split_ratio_from_one_tenth_to_nine_tenths(text: &str) -> Result<f32, String> {
    let split_ratio = parse_finite_number(text)?;
    if !(0.1..=0.9).contains(&split_ratio) {
        return Err(format!("the ratio {split_ratio} is not from 0.1 to 0.9"));
    }
    Ok(split_ratio)
}

pub(crate) fn parse_finite_non_negative_duration_in_seconds(text: &str) -> Result<f32, String> {
    let duration = parse_finite_number(text)?;
    if duration < 0.0 {
        return Err(format!("the duration {duration} is negative"));
    }
    Ok(duration)
}

pub(crate) fn parse_positive_font_size_in_points(text: &str) -> Result<f32, String> {
    let font_size = parse_finite_number(text)?;
    if font_size <= 0.0 {
        return Err(format!("the font size {font_size} is not above 0"));
    }
    Ok(font_size)
}

#[cfg(test)]
pub(crate) fn assert_every_value_is_spelled_alike_on_the_command_line_and_in_json<Value>()
where
    Value: ValueEnum + Serialize + fmt::Debug,
{
    for value in Value::value_variants() {
        assert_eq!(
            serde_json::to_value(value).unwrap(),
            serde_json::Value::String(command_line_spelling_of(value)),
            "{value:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExternalBarPadding, OnOrOff, PackedArgbColor,
        parse_finite_non_negative_duration_in_seconds, parse_opacity_above_zero_up_to_one,
        parse_opacity_from_zero_to_one, parse_packed_argb_color_other_than_zero,
        parse_positive_font_size_in_points, parse_split_ratio_from_one_tenth_to_nine_tenths,
    };
    use crate::display::manager::ExternalBarMode;

    #[test]
    fn a_packed_argb_color_is_0x_then_one_to_eight_hexadecimal_digits_in_either_case() {
        assert_eq!("0xAA336699".parse(), Ok(PackedArgbColor(0xaa336699)));
        assert_eq!("0Xff".parse(), Ok(PackedArgbColor(0xff)));
        for refused in [
            "",
            "0x",
            "ff336699",
            "4278190335",
            "0x1ffffffff",
            "0x+f",
            "0xgg",
            "red",
        ] {
            assert!(refused.parse::<PackedArgbColor>().is_err(), "{refused:?}");
        }
    }

    #[test]
    fn a_packed_argb_color_prints_and_crosses_the_socket_with_all_eight_digits() {
        let color = PackedArgbColor(0x00292e42);

        assert_eq!(color.to_string(), "0x00292e42");
        assert_eq!(serde_json::to_string(&color).unwrap(), "\"0x00292e42\"");
        assert_eq!(
            serde_json::from_str::<PackedArgbColor>("\"0x00292e42\"").unwrap(),
            color
        );
    }

    #[test]
    fn a_colour_that_must_show_refuses_zero_and_only_zero() {
        assert!(parse_packed_argb_color_other_than_zero("0x0").is_err());
        assert!(parse_packed_argb_color_other_than_zero("0x00000000").is_err());
        assert_eq!(
            parse_packed_argb_color_other_than_zero("0x00292e42"),
            Ok(PackedArgbColor(0x00292e42))
        );
    }

    #[test]
    fn an_external_bar_is_a_mode_then_a_top_and_a_bottom_padding() {
        assert_eq!(
            "all:40:0".parse(),
            Ok(ExternalBarPadding {
                mode: ExternalBarMode::EveryDisplay,
                top_padding: 40,
                bottom_padding: 0,
            })
        );
        assert_eq!(
            "main:0:12"
                .parse::<ExternalBarPadding>()
                .unwrap()
                .to_string(),
            "main:0:12"
        );
        for refused in [
            "all:40",
            "all:40:0:0",
            "top:40:0",
            "all:-1:0",
            "all:a:0",
            "off",
        ] {
            assert!(
                refused.parse::<ExternalBarPadding>().is_err(),
                "{refused:?}"
            );
        }
    }

    #[test]
    fn number_settings_accept_an_integer_spelling_and_refuse_what_is_out_of_range() {
        assert_eq!(parse_opacity_from_zero_to_one("1"), Ok(1.0));
        assert_eq!(parse_opacity_from_zero_to_one("0"), Ok(0.0));
        assert!(parse_opacity_from_zero_to_one("1.01").is_err());
        assert!(parse_opacity_above_zero_up_to_one("0").is_err());
        assert_eq!(parse_opacity_above_zero_up_to_one("0.9"), Ok(0.9));
        assert!(parse_split_ratio_from_one_tenth_to_nine_tenths("0.95").is_err());
        assert_eq!(
            parse_split_ratio_from_one_tenth_to_nine_tenths("0.5"),
            Ok(0.5)
        );
        assert!(parse_finite_non_negative_duration_in_seconds("-0.1").is_err());
        assert_eq!(parse_finite_non_negative_duration_in_seconds("0"), Ok(0.0));
        assert!(parse_positive_font_size_in_points("0").is_err());
        assert_eq!(parse_positive_font_size_in_points("13"), Ok(13.0));
    }

    #[test]
    fn number_settings_refuse_what_is_not_a_finite_number() {
        for refused in ["", "nan", "inf", "-inf", "one", "0.5.1"] {
            assert!(
                parse_finite_non_negative_duration_in_seconds(refused).is_err(),
                "{refused:?}"
            );
        }
    }

    #[test]
    fn a_switch_turns_into_on_or_off_and_back() {
        assert_eq!(OnOrOff::of_switch(true), OnOrOff::On);
        assert!(!OnOrOff::of_switch(false).is_on());
    }
}
