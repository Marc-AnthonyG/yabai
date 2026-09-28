use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use crate::ffi::skylight::SLSWindowIsOrderedIn;
use crate::scripting_addition::frame::{ScriptingAdditionFrame, ScriptingAdditionOpcode};
use crate::state::process_wide::{SCRIPTING_ADDITION_SOCKET_PATH, SKYLIGHT_CONNECTION_ID};
use crate::support::handles::{SpaceId, WindowId};
use crate::window::proxy_pairing::WindowProxyPairing;

fn connect_to_the_scripting_addition() -> Option<UnixStream> {
    UnixStream::connect(
        SCRIPTING_ADDITION_SOCKET_PATH
            .get()
            .map_or("", String::as_str),
    )
    .ok()
}

pub(crate) fn request_scripting_addition_handshake(
    version: &mut String,
    attributes: &mut u32,
) -> bool {
    let Some(mut stream) = connect_to_the_scripting_addition() else {
        return false;
    };
    let request = [0x01, 0x00, ScriptingAdditionOpcode::Handshake as u8];
    if stream.write_all(&request).is_err() {
        return false;
    }

    let mut response = [0u8; libc::BUFSIZ as usize];
    let last_byte_left_as_the_terminator = response.len() - 1;
    match stream.read(&mut response[..last_byte_left_as_the_terminator]) {
        Ok(length) if length > 0 => {}
        _ => return false,
    }

    let Some(version_length) = response.iter().position(|byte| *byte == 0) else {
        return false;
    };
    let Some(attribute_bytes) =
        response.get(version_length + 1..version_length + 1 + size_of::<u32>())
    else {
        return false;
    };
    *version = String::from_utf8_lossy(&response[..version_length]).into_owned();
    *attributes = u32::from_ne_bytes(attribute_bytes.try_into().unwrap());
    true
}

pub(crate) fn send_frame_to_scripting_addition_and_wait_for_acknowledgement(bytes: &[u8]) -> bool {
    let Some(mut stream) = connect_to_the_scripting_addition() else {
        return false;
    };
    if stream.write_all(bytes).is_err() {
        return false;
    }
    let _ = stream.read(&mut [0u8; 1]);
    true
}

pub(crate) fn focus_space_through_scripting_addition(space_id: SpaceId) -> bool {
    ScriptingAdditionFrame::new()
        .append(&space_id.0.to_ne_bytes())
        .send_as(ScriptingAdditionOpcode::SpaceFocus)
}

pub(crate) fn create_space_on_display_of_space_through_scripting_addition(
    space_id: SpaceId,
) -> bool {
    ScriptingAdditionFrame::new()
        .append(&space_id.0.to_ne_bytes())
        .send_as(ScriptingAdditionOpcode::SpaceCreate)
}

pub(crate) fn destroy_space_through_scripting_addition(space_id: SpaceId) -> bool {
    ScriptingAdditionFrame::new()
        .append(&space_id.0.to_ne_bytes())
        .send_as(ScriptingAdditionOpcode::SpaceDestroy)
}

pub(crate) fn move_space_to_display_through_scripting_addition(
    source_space_id: SpaceId,
    destination_space_id: SpaceId,
    source_previous_space_id: SpaceId,
    focus: bool,
) -> bool {
    ScriptingAdditionFrame::new()
        .append(&source_space_id.0.to_ne_bytes())
        .append(&destination_space_id.0.to_ne_bytes())
        .append(&source_previous_space_id.0.to_ne_bytes())
        .append(&[focus as u8])
        .send_as(ScriptingAdditionOpcode::SpaceMove)
}

pub(crate) fn move_space_after_space_through_scripting_addition(
    source_space_id: SpaceId,
    destination_space_id: SpaceId,
    focus: bool,
) -> bool {
    let no_previous_space_id: u64 = 0;
    ScriptingAdditionFrame::new()
        .append(&source_space_id.0.to_ne_bytes())
        .append(&destination_space_id.0.to_ne_bytes())
        .append(&no_previous_space_id.to_ne_bytes())
        .append(&[focus as u8])
        .send_as(ScriptingAdditionOpcode::SpaceMove)
}

pub(crate) fn move_window_through_scripting_addition(window_id: WindowId, x: i32, y: i32) -> bool {
    ScriptingAdditionFrame::new()
        .append(&window_id.0.to_ne_bytes())
        .append(&x.to_ne_bytes())
        .append(&y.to_ne_bytes())
        .send_as(ScriptingAdditionOpcode::WindowMove)
}

pub(crate) fn set_window_opacity_through_scripting_addition(
    window_id: WindowId,
    opacity: f32,
    duration: f32,
) -> bool {
    ScriptingAdditionFrame::new()
        .append(&window_id.0.to_ne_bytes())
        .append(&opacity.to_ne_bytes())
        .append(&duration.to_ne_bytes())
        .send_as(if duration > 0.0f32 {
            ScriptingAdditionOpcode::WindowOpacityFade
        } else {
            ScriptingAdditionOpcode::WindowOpacity
        })
}

pub(crate) fn set_window_layer_through_scripting_addition(window_id: WindowId, layer: i32) -> bool {
    ScriptingAdditionFrame::new()
        .append(&window_id.0.to_ne_bytes())
        .append(&layer.to_ne_bytes())
        .send_as(ScriptingAdditionOpcode::WindowLayer)
}

pub(crate) fn set_window_sticky_through_scripting_addition(
    window_id: WindowId,
    sticky: bool,
) -> bool {
    ScriptingAdditionFrame::new()
        .append(&window_id.0.to_ne_bytes())
        .append(&[sticky as u8])
        .send_as(ScriptingAdditionOpcode::WindowSticky)
}

pub(crate) fn set_window_shadow_through_scripting_addition(
    window_id: WindowId,
    shadow: bool,
) -> bool {
    ScriptingAdditionFrame::new()
        .append(&window_id.0.to_ne_bytes())
        .append(&[shadow as u8])
        .send_as(ScriptingAdditionOpcode::WindowShadow)
}

pub(crate) fn scale_window_through_scripting_addition(
    window_id: WindowId,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> bool {
    ScriptingAdditionFrame::new()
        .append(&window_id.0.to_ne_bytes())
        .append(&x.to_ne_bytes())
        .append(&y.to_ne_bytes())
        .append(&width.to_ne_bytes())
        .append(&height.to_ne_bytes())
        .send_as(ScriptingAdditionOpcode::WindowScale)
}

fn append_window_proxy_pairings<'frame>(
    frame: &'frame mut ScriptingAdditionFrame,
    pairing_list: &[WindowProxyPairing],
) -> &'frame mut ScriptingAdditionFrame {
    frame.append(&(pairing_list.len() as i32).to_ne_bytes());
    for pairing in pairing_list {
        frame
            .append(&pairing.real_window_id.0.to_ne_bytes())
            .append(&pairing.proxy_window_id.to_ne_bytes());
    }
    frame
}

pub(crate) fn swap_window_proxies_in_through_scripting_addition(
    pairing_list: &[WindowProxyPairing],
) -> bool {
    append_window_proxy_pairings(&mut ScriptingAdditionFrame::new(), pairing_list)
        .send_as(ScriptingAdditionOpcode::WindowSwapProxyIn)
}

pub(crate) fn swap_window_proxies_out_through_scripting_addition(
    pairing_list: &[WindowProxyPairing],
) -> bool {
    append_window_proxy_pairings(&mut ScriptingAdditionFrame::new(), pairing_list)
        .send_as(ScriptingAdditionOpcode::WindowSwapProxyOut)
}

pub(crate) fn order_window_relative_to_other_window_through_scripting_addition(
    a_window_id: WindowId,
    order: i32,
    b_window_id: WindowId,
) -> bool {
    ScriptingAdditionFrame::new()
        .append(&a_window_id.0.to_ne_bytes())
        .append(&order.to_ne_bytes())
        .append(&b_window_id.0.to_ne_bytes())
        .send_as(ScriptingAdditionOpcode::WindowOrder)
}

fn is_window_ordered_in(window_id: WindowId) -> bool {
    let mut ordered_in: u8 = 0;
    unsafe {
        SLSWindowIsOrderedIn(
            *SKYLIGHT_CONNECTION_ID.get().unwrap(),
            window_id.0,
            &mut ordered_in,
        )
    };
    ordered_in != 0
}

pub(crate) fn order_in_windows_not_yet_ordered_in_through_scripting_addition(
    window_list: &[WindowId],
) -> bool {
    let window_id_the_payload_skips: u32 = 0;
    let mut frame = ScriptingAdditionFrame::new();
    frame.append(&(window_list.len() as i32).to_ne_bytes());
    for window_id in window_list {
        if is_window_ordered_in(*window_id) {
            frame.append(&window_id_the_payload_skips.to_ne_bytes());
        } else {
            frame.append(&window_id.0.to_ne_bytes());
        }
    }
    frame.send_as(ScriptingAdditionOpcode::WindowOrderIn)
}

pub(crate) fn move_window_list_to_space_through_scripting_addition(
    space_id: SpaceId,
    window_list: &[WindowId],
) -> bool {
    let mut frame = ScriptingAdditionFrame::new();
    frame
        .append(&space_id.0.to_ne_bytes())
        .append(&(window_list.len() as i32).to_ne_bytes());
    for window_id in window_list {
        frame.append(&window_id.0.to_ne_bytes());
    }
    frame.send_as(ScriptingAdditionOpcode::WindowListToSpace)
}

pub(crate) fn move_window_to_space_through_scripting_addition(
    space_id: SpaceId,
    window_id: WindowId,
) -> bool {
    ScriptingAdditionFrame::new()
        .append(&space_id.0.to_ne_bytes())
        .append(&window_id.0.to_ne_bytes())
        .send_as(ScriptingAdditionOpcode::WindowToSpace)
}

#[cfg(test)]
mod tests {
    use super::append_window_proxy_pairings;
    use crate::scripting_addition::frame::{
        SCRIPTING_ADDITION_FRAME_BUFFER_LENGTH, ScriptingAdditionFrame, ScriptingAdditionOpcode,
    };
    use crate::support::handles::WindowId;
    use crate::window::proxy_pairing::WindowProxyPairing;

    const LENGTH_OF_THE_HEADER_AND_OPCODE: usize = 1 + 2;

    fn pairing(real_window_id: u32, proxy_window_id: u32) -> WindowProxyPairing {
        WindowProxyPairing {
            real_window_id: WindowId(real_window_id),
            proxy_window_id,
        }
    }

    fn packed_payload_of(pairing_list: &[WindowProxyPairing]) -> Option<Vec<u8>> {
        append_window_proxy_pairings(&mut ScriptingAdditionFrame::new(), pairing_list)
            .finish_with_opcode(ScriptingAdditionOpcode::WindowSwapProxyIn)
            .map(|frame| frame[LENGTH_OF_THE_HEADER_AND_OPCODE..].to_vec())
    }

    #[test]
    fn the_pairings_are_packed_as_an_int_count_then_each_real_window_id_before_its_proxy_window_id()
    {
        assert_eq!(
            packed_payload_of(&[pairing(0x11223344, 0xaabbccdd), pairing(7, 0x0100)]),
            Some(vec![
                0x02, 0x00, 0x00, 0x00, 0x44, 0x33, 0x22, 0x11, 0xdd, 0xcc, 0xbb, 0xaa, 0x07, 0x00,
                0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
            ])
        );
    }

    #[test]
    fn no_pairing_packs_a_zero_count_the_payload_returns_on() {
        assert_eq!(packed_payload_of(&[]), Some(vec![0x00, 0x00, 0x00, 0x00]));
    }

    #[test]
    fn as_many_pairings_as_the_frame_holds_are_packed_and_one_more_is_refused() {
        let pairings_the_frame_holds: Vec<WindowProxyPairing> = (1..=511)
            .map(|window_id| pairing(window_id, window_id + 5000))
            .collect();
        let one_pairing_too_many: Vec<WindowProxyPairing> = (1..=512)
            .map(|window_id| pairing(window_id, window_id + 5000))
            .collect();

        let packed_payload = packed_payload_of(&pairings_the_frame_holds).unwrap();

        assert_eq!(
            LENGTH_OF_THE_HEADER_AND_OPCODE + packed_payload.len(),
            SCRIPTING_ADDITION_FRAME_BUFFER_LENGTH - 1
        );
        assert_eq!(packed_payload[..4], 511i32.to_ne_bytes());
        assert_eq!(
            packed_payload[packed_payload.len() - 8..packed_payload.len() - 4],
            511u32.to_ne_bytes()
        );
        assert_eq!(
            packed_payload[packed_payload.len() - 4..],
            5511u32.to_ne_bytes()
        );
        assert!(packed_payload_of(&one_pairing_too_many).is_none());
    }
}
