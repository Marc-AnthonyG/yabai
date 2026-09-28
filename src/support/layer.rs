use clap::ValueEnum;
use serde::{Deserialize, Serialize};

pub(crate) const LAYER_AUTO: i32 = 0;
pub(crate) const LAYER_BELOW: i32 = 3;
pub(crate) const LAYER_NORMAL: i32 = 4;
pub(crate) const LAYER_ABOVE: i32 = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WindowStackingSubLayer {
    Below,
    Normal,
    Above,
    Auto,
}

impl WindowStackingSubLayer {
    pub(crate) fn layer(self) -> i32 {
        match self {
            WindowStackingSubLayer::Below => LAYER_BELOW,
            WindowStackingSubLayer::Normal => LAYER_NORMAL,
            WindowStackingSubLayer::Above => LAYER_ABOVE,
            WindowStackingSubLayer::Auto => LAYER_AUTO,
        }
    }

    pub(crate) fn of_layer(layer: i32) -> Option<WindowStackingSubLayer> {
        match layer {
            LAYER_BELOW => Some(WindowStackingSubLayer::Below),
            LAYER_NORMAL => Some(WindowStackingSubLayer::Normal),
            LAYER_ABOVE => Some(WindowStackingSubLayer::Above),
            LAYER_AUTO => Some(WindowStackingSubLayer::Auto),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum;

    use super::WindowStackingSubLayer;
    use crate::command::values::assert_every_value_is_spelled_alike_on_the_command_line_and_in_json;

    #[test]
    fn every_sub_layer_turns_into_its_layer_and_back() {
        for sub_layer in WindowStackingSubLayer::value_variants() {
            assert_eq!(
                WindowStackingSubLayer::of_layer(sub_layer.layer()),
                Some(*sub_layer)
            );
        }
    }

    #[test]
    fn every_sub_layer_is_spelled_alike_on_the_command_line_and_in_json() {
        assert_every_value_is_spelled_alike_on_the_command_line_and_in_json::<WindowStackingSubLayer>(
        );
    }
}
