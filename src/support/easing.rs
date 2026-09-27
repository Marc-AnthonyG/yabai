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
