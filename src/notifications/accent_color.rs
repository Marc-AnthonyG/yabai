use crate::event::queue::{Event, event_loop_post};
use crate::ffi::appkit::{NSColor, NSColorSpace};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::support::color::{RgbaColor, rgba_color_from_hex};

pub(crate) fn read_the_system_accent_color_as_srgb() -> Option<RgbaColor> {
    let accent_color_in_srgb =
        NSColor::controlAccentColor().colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;

    Some(rgba_color_from_hex(packed_argb_from_srgb_components(
        accent_color_in_srgb.redComponent(),
        accent_color_in_srgb.greenComponent(),
        accent_color_in_srgb.blueComponent(),
        accent_color_in_srgb.alphaComponent(),
    )))
}

fn packed_argb_from_srgb_components(red: f64, green: f64, blue: f64, alpha: f64) -> u32 {
    (color_component_scaled_to_a_byte(alpha) << 0x18)
        | (color_component_scaled_to_a_byte(red) << 0x10)
        | (color_component_scaled_to_a_byte(green) << 0x08)
        | color_component_scaled_to_a_byte(blue)
}

fn color_component_scaled_to_a_byte(color_component: f64) -> u32 {
    (color_component.clamp(0.0, 1.0) * 255.0).round() as u32
}

pub(crate) fn post_the_system_accent_color_to_the_event_loop() {
    if let Some(accent_color) = read_the_system_accent_color_as_srgb() {
        event_loop_post(Event::SystemAccentColorChanged(accent_color));
    }
}

pub(crate) fn post_the_system_accent_color_once_every_other_observer_has_seen_the_change() {
    dispatch_after_on_main_queue(0, post_the_system_accent_color_to_the_event_loop);
}

#[cfg(test)]
mod tests {
    use super::{color_component_scaled_to_a_byte, packed_argb_from_srgb_components};
    use crate::support::color::rgba_color_from_hex;

    #[test]
    fn a_component_is_scaled_to_the_nearest_byte_rounding_halves_up() {
        let expected_bytes = [
            (0.0, 0),
            (1.0, 255),
            (0.125, 32),
            (0.25, 64),
            (0.375, 96),
            (0.5, 128),
            (0.75, 191),
            (0.2, 51),
            (122.0 / 255.0, 122),
            (122.49 / 255.0, 122),
            (122.51 / 255.0, 123),
        ];

        for (color_component, expected_byte) in expected_bytes {
            assert_eq!(
                color_component_scaled_to_a_byte(color_component),
                expected_byte,
                "component {color_component}"
            );
        }
    }

    #[test]
    fn a_component_outside_zero_to_one_is_clamped_and_one_that_is_not_a_number_scales_to_zero() {
        let expected_bytes = [
            (-0.001, 0),
            (-1e9, 0),
            (f64::NEG_INFINITY, 0),
            (1.000001, 255),
            (2.0, 255),
            (f64::INFINITY, 255),
            (f64::NAN, 0),
        ];

        for (color_component, expected_byte) in expected_bytes {
            assert_eq!(
                color_component_scaled_to_a_byte(color_component),
                expected_byte,
                "component {color_component}"
            );
        }
    }

    #[test]
    fn the_components_are_packed_with_alpha_in_the_top_byte_then_red_green_and_blue() {
        assert_eq!(
            packed_argb_from_srgb_components(1.0, 0.5, 0.25, 0.75),
            0xbfff8040
        );
        assert_eq!(
            packed_argb_from_srgb_components(0.0, 122.0 / 255.0, 1.0, 1.0),
            0xff007aff
        );
        assert_eq!(
            packed_argb_from_srgb_components(0.0, 0.0, 0.0, 0.0),
            0x00000000
        );
        assert_eq!(
            packed_argb_from_srgb_components(1.0, 1.0, 1.0, 1.0),
            0xffffffff
        );
    }

    #[test]
    fn a_packed_accent_colour_unpacks_to_the_components_it_was_packed_from() {
        let byte_exact_components = [
            (0x12, 0x34, 0x56, 0x78),
            (0xff, 0x00, 0x7a, 0xff),
            (1, 2, 254, 128),
        ];

        for (red_byte, green_byte, blue_byte, alpha_byte) in byte_exact_components {
            let color = rgba_color_from_hex(packed_argb_from_srgb_components(
                red_byte as f64 / 255.0,
                green_byte as f64 / 255.0,
                blue_byte as f64 / 255.0,
                alpha_byte as f64 / 255.0,
            ));

            assert_eq!(
                [color.red, color.green, color.blue, color.alpha],
                [
                    red_byte as f32 / 255.0,
                    green_byte as f32 / 255.0,
                    blue_byte as f32 / 255.0,
                    alpha_byte as f32 / 255.0,
                ],
                "bytes {red_byte:#04x} {green_byte:#04x} {blue_byte:#04x} {alpha_byte:#04x}"
            );
        }
    }
}
