macro_rules! define_animation_easing_types {
    ($(($variant:ident, $name:literal, $function:path)),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub enum AnimationEasingType {
            $($variant),*
        }

        const EVERY_ANIMATION_EASING_TYPE: [AnimationEasingType; ANIMATION_EASING_TYPE_COUNT] =
            [$(AnimationEasingType::$variant),*];

        pub static ANIMATION_EASING_TYPE_NAMES: [&str; ANIMATION_EASING_TYPE_COUNT] = [$($name),*];

        impl AnimationEasingType {
            pub fn ease_interpolant(self, interpolant: f32) -> f32 {
                match self {
                    $(AnimationEasingType::$variant => $function(interpolant)),*
                }
            }
        }
    };
}

define_animation_easing_types! {
    (EaseInSine, "ease_in_sine", simple_easing::sine_in),
    (EaseOutSine, "ease_out_sine", simple_easing::sine_out),
    (EaseInOutSine, "ease_in_out_sine", simple_easing::sine_in_out),
    (EaseInQuad, "ease_in_quad", simple_easing::quad_in),
    (EaseOutQuad, "ease_out_quad", simple_easing::quad_out),
    (EaseInOutQuad, "ease_in_out_quad", simple_easing::quad_in_out),
    (EaseInCubic, "ease_in_cubic", simple_easing::cubic_in),
    (EaseOutCubic, "ease_out_cubic", simple_easing::cubic_out),
    (EaseInOutCubic, "ease_in_out_cubic", simple_easing::cubic_in_out),
    (EaseInQuart, "ease_in_quart", simple_easing::quart_in),
    (EaseOutQuart, "ease_out_quart", simple_easing::quart_out),
    (EaseInOutQuart, "ease_in_out_quart", simple_easing::quart_in_out),
    (EaseInQuint, "ease_in_quint", simple_easing::quint_in),
    (EaseOutQuint, "ease_out_quint", simple_easing::quint_out),
    (EaseInOutQuint, "ease_in_out_quint", simple_easing::quint_in_out),
    (EaseInExpo, "ease_in_expo", simple_easing::expo_in),
    (EaseOutExpo, "ease_out_expo", simple_easing::expo_out),
    (EaseInOutExpo, "ease_in_out_expo", simple_easing::expo_in_out),
    (EaseInCirc, "ease_in_circ", simple_easing::circ_in),
    (EaseOutCirc, "ease_out_circ", simple_easing::circ_out),
    (EaseInOutCirc, "ease_in_out_circ", simple_easing::circ_in_out),
}

pub const ANIMATION_EASING_TYPE_COUNT: usize = 21;

impl AnimationEasingType {
    pub fn from_index(index: usize) -> Option<AnimationEasingType> {
        EVERY_ANIMATION_EASING_TYPE.get(index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::{ANIMATION_EASING_TYPE_COUNT, ANIMATION_EASING_TYPE_NAMES, AnimationEasingType};

    fn every_easing() -> impl Iterator<Item = (AnimationEasingType, &'static str)> {
        (0..ANIMATION_EASING_TYPE_COUNT).map(|index| {
            let easing = AnimationEasingType::from_index(index)
                .unwrap_or_else(|| panic!("easing index {index} should exist"));
            (easing, ANIMATION_EASING_TYPE_NAMES[index])
        })
    }

    #[test]
    fn each_easing_index_round_trips_through_its_discriminant() {
        for (index, (easing, name)) in every_easing().enumerate() {
            assert_eq!(easing as usize, index, "{name}");
        }
        assert!(AnimationEasingType::from_index(ANIMATION_EASING_TYPE_COUNT).is_none());
    }

    #[test]
    fn each_easing_starts_at_zero_and_comes_to_rest_at_one() {
        for (easing, name) in every_easing() {
            assert!(easing.ease_interpolant(0.0).abs() < 1e-6, "{name} at 0");
            assert!(
                (easing.ease_interpolant(1.0) - 1.0).abs() < 1e-6,
                "{name} at 1"
            );
        }
    }

    #[test]
    fn the_configured_names_keep_their_spelling() {
        assert_eq!(ANIMATION_EASING_TYPE_NAMES[0], "ease_in_sine");
        assert_eq!(
            ANIMATION_EASING_TYPE_NAMES[AnimationEasingType::EaseOutCirc as usize],
            "ease_out_circ"
        );
        assert_eq!(
            ANIMATION_EASING_TYPE_NAMES[ANIMATION_EASING_TYPE_COUNT - 1],
            "ease_in_out_circ"
        );
    }
}
