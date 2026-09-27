macro_rules! animation_easing_type_list {
    ($animation_easing_type_entry:ident) => {
        $animation_easing_type_entry! {
            (EaseInSine, ease_in_sine, 0),
            (EaseOutSine, ease_out_sine, 1),
            (EaseInOutSine, ease_in_out_sine, 2),
            (EaseInQuad, ease_in_quad, 3),
            (EaseOutQuad, ease_out_quad, 4),
            (EaseInOutQuad, ease_in_out_quad, 5),
            (EaseInCubic, ease_in_cubic, 6),
            (EaseOutCubic, ease_out_cubic, 7),
            (EaseInOutCubic, ease_in_out_cubic, 8),
            (EaseInQuart, ease_in_quart, 9),
            (EaseOutQuart, ease_out_quart, 10),
            (EaseInOutQuart, ease_in_out_quart, 11),
            (EaseInQuint, ease_in_quint, 12),
            (EaseOutQuint, ease_out_quint, 13),
            (EaseInOutQuint, ease_in_out_quint, 14),
            (EaseInExpo, ease_in_expo, 15),
            (EaseOutExpo, ease_out_expo, 16),
            (EaseInOutExpo, ease_in_out_expo, 17),
            (EaseInCirc, ease_in_circ, 18),
            (EaseOutCirc, ease_out_circ, 19),
            (EaseInOutCirc, ease_in_out_circ, 20),
        }
    };
}

macro_rules! define_animation_easing_type {
    ($(($variant:ident, $function:ident, $value:literal)),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(usize)]
        pub enum AnimationEasingType {
            $($variant = $value),*
        }

        pub static ANIMATION_EASING_TYPE_STR: [&str; EASING_TYPE_COUNT] =
            [$(stringify!($function)),*];

        impl AnimationEasingType {
            pub fn apply(self, interpolant: f32) -> f32 {
                match self {
                    $(AnimationEasingType::$variant => $function(interpolant)),*
                }
            }

            pub fn from_index(index: usize) -> Option<AnimationEasingType> {
                match index {
                    $($value => Some(AnimationEasingType::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

animation_easing_type_list!(define_animation_easing_type);

pub const EASING_TYPE_COUNT: usize = 21;

pub fn ease_in_sine(interpolant: f32) -> f32 {
    1.0 - (((interpolant as f64 * std::f64::consts::PI) / 2.0) as f32).cos()
}

pub fn ease_out_sine(interpolant: f32) -> f32 {
    (((interpolant as f64 * std::f64::consts::PI) / 2.0) as f32).sin()
}

pub fn ease_in_out_sine(interpolant: f32) -> f32 {
    -(((std::f64::consts::PI * interpolant as f64) as f32).cos() - 1.0) / 2.0
}

pub fn ease_in_quad(interpolant: f32) -> f32 {
    interpolant * interpolant
}

pub fn ease_out_quad(interpolant: f32) -> f32 {
    1.0 - (1.0 - interpolant) * (1.0 - interpolant)
}

pub fn ease_in_out_quad(interpolant: f32) -> f32 {
    if interpolant < 0.5 {
        2.0 * interpolant * interpolant
    } else {
        1.0 - (-2.0 * interpolant + 2.0).powf(2.0) / 2.0
    }
}

pub fn ease_in_cubic(interpolant: f32) -> f32 {
    interpolant * interpolant * interpolant
}

pub fn ease_out_cubic(interpolant: f32) -> f32 {
    1.0 - (1.0 - interpolant).powf(3.0)
}

pub fn ease_in_out_cubic(interpolant: f32) -> f32 {
    if interpolant < 0.5 {
        4.0 * interpolant * interpolant * interpolant
    } else {
        1.0 - (-2.0 * interpolant + 2.0).powf(3.0) / 2.0
    }
}

pub fn ease_in_quart(interpolant: f32) -> f32 {
    interpolant * interpolant * interpolant * interpolant
}

pub fn ease_out_quart(interpolant: f32) -> f32 {
    1.0 - (1.0 - interpolant).powf(4.0)
}

pub fn ease_in_out_quart(interpolant: f32) -> f32 {
    if interpolant < 0.5 {
        8.0 * interpolant * interpolant * interpolant * interpolant
    } else {
        1.0 - (-2.0 * interpolant + 2.0).powf(4.0) / 2.0
    }
}

pub fn ease_in_quint(interpolant: f32) -> f32 {
    interpolant * interpolant * interpolant * interpolant * interpolant
}

pub fn ease_out_quint(interpolant: f32) -> f32 {
    1.0 - (1.0 - interpolant).powf(5.0)
}

pub fn ease_in_out_quint(interpolant: f32) -> f32 {
    if interpolant < 0.5 {
        16.0 * interpolant * interpolant * interpolant * interpolant * interpolant
    } else {
        1.0 - (-2.0 * interpolant + 2.0).powf(5.0) / 2.0
    }
}

pub fn ease_in_expo(interpolant: f32) -> f32 {
    if interpolant == 0.0 {
        0.0
    } else {
        2.0f32.powf(10.0 * interpolant - 10.0)
    }
}

pub fn ease_out_expo(interpolant: f32) -> f32 {
    if interpolant == 1.0 {
        1.0
    } else {
        1.0 - 2.0f32.powf(-10.0 * interpolant)
    }
}

pub fn ease_in_out_expo(interpolant: f32) -> f32 {
    if interpolant == 0.0 {
        0.0
    } else if interpolant == 1.0 {
        1.0
    } else if interpolant < 0.5 {
        2.0f32.powf(20.0 * interpolant - 10.0) / 2.0
    } else {
        (2.0 - 2.0f32.powf(-20.0 * interpolant + 10.0)) / 2.0
    }
}

pub fn ease_in_circ(interpolant: f32) -> f32 {
    1.0 - (1.0 - interpolant.powf(2.0)).sqrt()
}

pub fn ease_out_circ(interpolant: f32) -> f32 {
    (1.0 - (interpolant - 1.0).powf(2.0)).sqrt()
}

pub fn ease_in_out_circ(interpolant: f32) -> f32 {
    if interpolant < 0.5 {
        (1.0 - (1.0 - (2.0 * interpolant).powf(2.0)).sqrt()) / 2.0
    } else {
        ((1.0 - (-2.0 * interpolant + 2.0).powf(2.0)).sqrt() + 1.0) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::{ANIMATION_EASING_TYPE_STR, AnimationEasingType, EASING_TYPE_COUNT};

    const INTERPOLANTS_WHERE_EVERY_C_BUILD_AGREES: [f32; 6] = [0.0, 0.25, 0.3333, 0.5, 0.75, 1.0];

    const C_EASING_RESULT_BITS_IN_ENUM_ORDER: [(&str, [u32; 6]); EASING_TYPE_COUNT] = [
        (
            "ease_in_sine",
            [
                0x00000000, 0x3d9be510, 0x3e0929c4, 0x3e95f61a, 0x3f1e0876, 0x3f800000,
            ],
        ),
        (
            "ease_out_sine",
            [
                0x00000000, 0x3ec3ef16, 0x3efffa0e, 0x3f3504f3, 0x3f6c835e, 0x3f800000,
            ],
        ),
        (
            "ease_in_out_sine",
            [
                0x80000000, 0x3e15f61a, 0x3e7ff41c, 0x3f000000, 0x3f5a827a, 0x3f800000,
            ],
        ),
        (
            "ease_in_quad",
            [
                0x00000000, 0x3d800000, 0x3de38292, 0x3e800000, 0x3f100000, 0x3f800000,
            ],
        ),
        (
            "ease_out_quad",
            [
                0x00000000, 0x3ee00000, 0x3f0e35fa, 0x3f400000, 0x3f700000, 0x3f800000,
            ],
        ),
        (
            "ease_in_out_quad",
            [
                0x00000000, 0x3e000000, 0x3e638292, 0x3f000000, 0x3f600000, 0x3f800000,
            ],
        ),
        (
            "ease_in_cubic",
            [
                0x00000000, 0x3c800000, 0x3d17a87f, 0x3e000000, 0x3ed80000, 0x3f800000,
            ],
        ),
        (
            "ease_out_cubic",
            [
                0x00000000, 0x3f140000, 0x3f342303, 0x3f600000, 0x3f7c0000, 0x3f800000,
            ],
        ),
        (
            "ease_in_out_cubic",
            [
                0x00000000, 0x3d800000, 0x3e17a87f, 0x3f000000, 0x3f700000, 0x3f800000,
            ],
        ),
        (
            "ease_in_quart",
            [
                0x00000000, 0x3b800000, 0x3c4a30d1, 0x3d800000, 0x3ea20000, 0x3f800000,
            ],
        ),
        (
            "ease_out_quart",
            [
                0x00000000, 0x3f2f0000, 0x3f4d6c07, 0x3f700000, 0x3f7f0000, 0x3f800000,
            ],
        ),
        (
            "ease_in_out_quart",
            [
                0x00000000, 0x3d000000, 0x3dca30d1, 0x3f000000, 0x3f780000, 0x3f800000,
            ],
        ),
        (
            "ease_in_quint",
            [
                0x00000000, 0x3a800000, 0x3b86c7c2, 0x3d000000, 0x3e730000, 0x3f800000,
            ],
        ),
        (
            "ease_out_quint",
            [
                0x00000000, 0x3f434000, 0x3f5e4796, 0x3f780000, 0x3f7fc000, 0x3f800000,
            ],
        ),
        (
            "ease_in_out_quint",
            [
                0x00000000, 0x3c800000, 0x3d86c7c2, 0x3f000000, 0x3f7c0000, 0x3f800000,
            ],
        ),
        (
            "ease_in_expo",
            [
                0x00000000, 0x3bb504f3, 0x3c213b8f, 0x3d000000, 0x3e3504f3, 0x3f800000,
            ],
        ),
        (
            "ease_out_expo",
            [
                0x00000000, 0x3f52bec3, 0x3f669881, 0x3f780000, 0x3f7e95f6, 0x3f800000,
            ],
        ),
        (
            "ease_in_out_expo",
            [
                0x00000000, 0x3c800000, 0x3d4b17ec, 0x3f000000, 0x3f7c0000, 0x3f800000,
            ],
        ),
        (
            "ease_in_circ",
            [
                0x00000000, 0x3d0210a0, 0x3d6a34b0, 0x3e0930a4, 0x3ead5806, 0x3f800000,
            ],
        ),
        (
            "ease_out_circ",
            [
                0x00000000, 0x3f2953fd, 0x3f3ecdb2, 0x3f5db3d7, 0x3f77def6, 0x3f800000,
            ],
        ),
        (
            "ease_in_out_circ",
            [
                0x00000000, 0x3d8930a4, 0x3e0258e2, 0x3f000000, 0x3f6ed9ec, 0x3f800000,
            ],
        ),
    ];

    #[test]
    fn easing_names_follow_the_c_enum_order_and_spelling() {
        let c_names: Vec<&str> = C_EASING_RESULT_BITS_IN_ENUM_ORDER
            .iter()
            .map(|(name, _)| *name)
            .collect();

        assert_eq!(ANIMATION_EASING_TYPE_STR.to_vec(), c_names);
    }

    #[test]
    fn each_easing_returns_the_same_f32_bits_as_the_c_function_of_the_same_index() {
        for (index, (name, expected_bits)) in C_EASING_RESULT_BITS_IN_ENUM_ORDER.iter().enumerate()
        {
            let easing = AnimationEasingType::from_index(index)
                .unwrap_or_else(|| panic!("easing index {index} should exist"));

            let result_bits: Vec<u32> = INTERPOLANTS_WHERE_EVERY_C_BUILD_AGREES
                .iter()
                .map(|interpolant| easing.apply(*interpolant).to_bits())
                .collect();

            assert_eq!(result_bits, expected_bits.to_vec(), "{name}");
        }
    }

    #[test]
    fn from_index_knows_exactly_the_twenty_one_c_easings() {
        assert_eq!(EASING_TYPE_COUNT, 21);
        assert!(AnimationEasingType::from_index(EASING_TYPE_COUNT - 1).is_some());
        assert!(AnimationEasingType::from_index(EASING_TYPE_COUNT).is_none());
    }

    #[test]
    fn each_easing_index_round_trips_through_its_discriminant() {
        for index in 0..EASING_TYPE_COUNT {
            let easing = AnimationEasingType::from_index(index)
                .unwrap_or_else(|| panic!("easing index {index} should exist"));

            assert_eq!(easing as usize, index);
        }
    }
}
