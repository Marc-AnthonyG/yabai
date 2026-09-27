use core::ffi::{c_char, c_void};

use crate::ffi::skylight::SLSWindowIsOrderedIn;
use crate::scripting_addition::frame::{SA_SOCKET_BUFF_LEN, SaOpcode, pack, sa_payload_send};
use crate::state::process_wide::{CONNECTION, SA_SOCKET_FILE};
use crate::support::handles::{SpaceId, WindowId};
use crate::support::sockets::{socket_close, socket_connect, socket_open};
use crate::window::proxy_pairing::WindowProxyPairing;

pub(crate) fn scripting_addition_request_handshake(
    version: &mut String,
    attributes: &mut u32,
) -> bool {
    let mut socket_file_descriptor: i32 = 0;
    let mut result = false;
    let mut response = [0u8; libc::BUFSIZ as usize];
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    bytes[0] = 0x01;
    bytes[1] = 0x00;
    bytes[2] = SaOpcode::Handshake as u8;

    if socket_open(&mut socket_file_descriptor) {
        'out: {
            if socket_connect(
                socket_file_descriptor,
                SA_SOCKET_FILE.get().map_or("", String::as_str),
            ) {
                if unsafe {
                    libc::send(
                        socket_file_descriptor,
                        bytes.as_ptr().cast::<c_void>(),
                        3,
                        0,
                    )
                } != -1
                {
                    let length = unsafe {
                        libc::recv(
                            socket_file_descriptor,
                            response.as_mut_ptr().cast::<c_void>(),
                            response.len() - 1,
                            0,
                        )
                    } as i32;
                    if length <= 0 {
                        break 'out;
                    }

                    let mut zero = 0;
                    while response[zero] != b'\0' {
                        zero += 1;
                    }

                    debug_assert!(response[zero] == b'\0');
                    let Some(attribute_bytes) = response.get(zero + 1..zero + 1 + size_of::<u32>())
                    else {
                        break 'out;
                    };
                    *version = String::from_utf8_lossy(&response[..zero]).into_owned();
                    *attributes = u32::from_ne_bytes(attribute_bytes.try_into().unwrap());

                    result = true;
                }
            }
        }

        socket_close(socket_file_descriptor);
    }

    result
}

pub(crate) fn scripting_addition_send_bytes(bytes: &[u8]) -> bool {
    let mut socket_file_descriptor: i32 = 0;
    let mut dummy: c_char = 0;
    let mut result = false;

    if socket_open(&mut socket_file_descriptor) {
        if socket_connect(
            socket_file_descriptor,
            SA_SOCKET_FILE.get().map_or("", String::as_str),
        ) {
            if unsafe {
                libc::send(
                    socket_file_descriptor,
                    bytes.as_ptr().cast::<c_void>(),
                    bytes.len(),
                    0,
                )
            } != -1
            {
                unsafe {
                    libc::recv(
                        socket_file_descriptor,
                        (&raw mut dummy).cast::<c_void>(),
                        1,
                        0,
                    )
                };
                result = true;
            }
        }

        socket_close(socket_file_descriptor);
    }

    result
}

pub(crate) fn scripting_addition_focus_space(space_id: SpaceId) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &space_id.0.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::SpaceFocus)
}

pub(crate) fn scripting_addition_create_space(space_id: SpaceId) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &space_id.0.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::SpaceCreate)
}

pub(crate) fn scripting_addition_destroy_space(space_id: SpaceId) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &space_id.0.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::SpaceDestroy)
}

pub(crate) fn scripting_addition_move_space_to_display(
    source_space_id: SpaceId,
    destination_space_id: SpaceId,
    source_previous_space_id: SpaceId,
    focus: bool,
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &source_space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &destination_space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &source_previous_space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &[focus as u8]) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::SpaceMove)
}

pub(crate) fn scripting_addition_move_space_after_space(
    source_space_id: SpaceId,
    destination_space_id: SpaceId,
    focus: bool,
) -> bool {
    let dummy_space_id: u64 = 0;
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &source_space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &destination_space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &dummy_space_id.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &[focus as u8]) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::SpaceMove)
}

pub(crate) fn scripting_addition_move_window(window_id: WindowId, x: i32, y: i32) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &x.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &y.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowMove)
}

pub(crate) fn scripting_addition_set_opacity(
    window_id: WindowId,
    opacity: f32,
    duration: f32,
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &opacity.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &duration.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(
        &mut bytes,
        length,
        if duration > 0.0f32 {
            SaOpcode::WindowOpacityFade
        } else {
            SaOpcode::WindowOpacity
        },
    )
}

pub(crate) fn scripting_addition_set_layer(window_id: WindowId, layer: i32) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &layer.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowLayer)
}

pub(crate) fn scripting_addition_set_sticky(window_id: WindowId, sticky: bool) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &[sticky as u8]) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowSticky)
}

pub(crate) fn scripting_addition_set_shadow(window_id: WindowId, shadow: bool) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &[shadow as u8]) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowShadow)
}

pub(crate) fn scripting_addition_scale_window(
    window_id: WindowId,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &x.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &y.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &width.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &height.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowScale)
}

fn pack_window_proxy_pairings(
    bytes: &mut [u8],
    length: &mut i16,
    pairing_list: &[WindowProxyPairing],
) -> bool {
    if !pack(bytes, length, &(pairing_list.len() as i32).to_ne_bytes()) {
        return false;
    }
    for pairing in pairing_list {
        if !pack(bytes, length, &pairing.real_window_id.0.to_ne_bytes()) {
            return false;
        }
        if !pack(bytes, length, &pairing.proxy_window_id.to_ne_bytes()) {
            return false;
        }
    }
    true
}

pub(crate) fn scripting_addition_swap_window_proxy_in(pairing_list: &[WindowProxyPairing]) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack_window_proxy_pairings(&mut bytes, &mut length, pairing_list) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowSwapProxyIn)
}

pub(crate) fn scripting_addition_swap_window_proxy_out(pairing_list: &[WindowProxyPairing]) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack_window_proxy_pairings(&mut bytes, &mut length, pairing_list) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowSwapProxyOut)
}

pub(crate) fn scripting_addition_order_window(
    a_window_id: WindowId,
    order: i32,
    b_window_id: WindowId,
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &a_window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &order.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &b_window_id.0.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowOrder)
}

pub(crate) fn scripting_addition_order_window_in(window_list: &[WindowId]) -> bool {
    let dummy_window_id: u32 = 0;
    let mut ordered_in: u8 = 0;

    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &(window_list.len() as i32).to_ne_bytes()) {
        return false;
    }
    for window_id in window_list {
        unsafe { SLSWindowIsOrderedIn(*CONNECTION.get().unwrap(), window_id.0, &mut ordered_in) };
        if ordered_in != 0 {
            if !pack(&mut bytes, &mut length, &dummy_window_id.to_ne_bytes()) {
                return false;
            }
        } else {
            if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
                return false;
            }
        }
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowOrderIn)
}

pub(crate) fn scripting_addition_move_window_list_to_space(
    space_id: SpaceId,
    window_list: &[WindowId],
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &(window_list.len() as i32).to_ne_bytes()) {
        return false;
    }
    for window_id in window_list {
        if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
            return false;
        }
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowListToSpace)
}

pub(crate) fn scripting_addition_move_window_to_space(
    space_id: SpaceId,
    window_id: WindowId,
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowToSpace)
}

#[cfg(test)]
mod tests {
    use super::pack_window_proxy_pairings;
    use crate::scripting_addition::frame::SA_SOCKET_BUFF_LEN;
    use crate::support::handles::WindowId;
    use crate::window::proxy_pairing::WindowProxyPairing;

    const LENGTH_OF_THE_HEADER_AND_OPCODE: i16 = 1 + 2;

    fn pairing(real_window_id: u32, proxy_window_id: u32) -> WindowProxyPairing {
        WindowProxyPairing {
            real_window_id: WindowId(real_window_id),
            proxy_window_id,
        }
    }

    fn packed_payload_of(pairing_list: &[WindowProxyPairing]) -> Option<Vec<u8>> {
        let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
        let mut length = LENGTH_OF_THE_HEADER_AND_OPCODE;
        pack_window_proxy_pairings(&mut bytes, &mut length, pairing_list)
            .then(|| bytes[LENGTH_OF_THE_HEADER_AND_OPCODE as usize..length as usize].to_vec())
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
            LENGTH_OF_THE_HEADER_AND_OPCODE as usize + packed_payload.len(),
            SA_SOCKET_BUFF_LEN - 1
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
