use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnimationEasingType {
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInQuint,
    EaseOutQuint,
    EaseInOutQuint,
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
    EaseInCirc,
    EaseOutCirc,
    EaseInOutCirc,
}

impl AnimationEasingType {
    pub fn ease_interpolant(self, interpolant: f32) -> f32 {
        match self {
            AnimationEasingType::EaseInSine => simple_easing::sine_in(interpolant),
            AnimationEasingType::EaseOutSine => simple_easing::sine_out(interpolant),
            AnimationEasingType::EaseInOutSine => simple_easing::sine_in_out(interpolant),
            AnimationEasingType::EaseInQuad => simple_easing::quad_in(interpolant),
            AnimationEasingType::EaseOutQuad => simple_easing::quad_out(interpolant),
            AnimationEasingType::EaseInOutQuad => simple_easing::quad_in_out(interpolant),
            AnimationEasingType::EaseInCubic => simple_easing::cubic_in(interpolant),
            AnimationEasingType::EaseOutCubic => simple_easing::cubic_out(interpolant),
            AnimationEasingType::EaseInOutCubic => simple_easing::cubic_in_out(interpolant),
            AnimationEasingType::EaseInQuart => simple_easing::quart_in(interpolant),
            AnimationEasingType::EaseOutQuart => simple_easing::quart_out(interpolant),
            AnimationEasingType::EaseInOutQuart => simple_easing::quart_in_out(interpolant),
            AnimationEasingType::EaseInQuint => simple_easing::quint_in(interpolant),
            AnimationEasingType::EaseOutQuint => simple_easing::quint_out(interpolant),
            AnimationEasingType::EaseInOutQuint => simple_easing::quint_in_out(interpolant),
            AnimationEasingType::EaseInExpo => simple_easing::expo_in(interpolant),
            AnimationEasingType::EaseOutExpo => simple_easing::expo_out(interpolant),
            AnimationEasingType::EaseInOutExpo => simple_easing::expo_in_out(interpolant),
            AnimationEasingType::EaseInCirc => simple_easing::circ_in(interpolant),
            AnimationEasingType::EaseOutCirc => simple_easing::circ_out(interpolant),
            AnimationEasingType::EaseInOutCirc => simple_easing::circ_in_out(interpolant),
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum;

    use super::AnimationEasingType;

    #[test]
    fn each_easing_starts_at_zero_and_comes_to_rest_at_one() {
        for easing in AnimationEasingType::value_variants() {
            assert!(easing.ease_interpolant(0.0).abs() < 1e-6, "{easing:?} at 0");
            assert!(
                (easing.ease_interpolant(1.0) - 1.0).abs() < 1e-6,
                "{easing:?} at 1"
            );
        }
    }

    #[test]
    fn an_easing_is_spelled_in_kebab_case_on_the_command_line_and_in_json() {
        let easing = AnimationEasingType::EaseInOutCirc;

        assert_eq!(
            easing.to_possible_value().unwrap().get_name(),
            "ease-in-out-circ"
        );
        assert_eq!(
            serde_json::to_string(&easing).unwrap(),
            "\"ease-in-out-circ\""
        );
    }
}
