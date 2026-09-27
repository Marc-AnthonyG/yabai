use crate::event::queue::{Event, event_loop_post};
use crate::ffi::appkit::{NSColor, NSColorSpace};
use crate::ffi::dispatch::dispatch_after_on_main_queue;
use crate::support::color::{RgbaColor, rgba_color_from_hex};

pub(crate) fn read_the_system_accent_color_as_srgb() -> Option<RgbaColor> {
    let accent_color_in_srgb =
        NSColor::controlAccentColor().colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;

    Some(rgba_color_from_hex(
        (color_component_scaled_to_a_byte(accent_color_in_srgb.alphaComponent()) << 0x18)
            | (color_component_scaled_to_a_byte(accent_color_in_srgb.redComponent()) << 0x10)
            | (color_component_scaled_to_a_byte(accent_color_in_srgb.greenComponent()) << 0x08)
            | color_component_scaled_to_a_byte(accent_color_in_srgb.blueComponent()),
    ))
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
