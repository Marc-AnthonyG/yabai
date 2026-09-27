use core::ffi::{c_char, c_void};
use std::sync::atomic::Ordering;

use crate::ffi::skylight::SLSWindowIsOrderedIn;
use crate::scripting_addition::frame::{SA_SOCKET_BUFF_LEN, SaOpcode, pack, sa_payload_send};
use crate::state::process_wide::{CONNECTION, SA_SOCKET_FILE};
use crate::support::handles::{SpaceId, WindowId};
use crate::support::sockets::{socket_close, socket_connect, socket_open};
use crate::window::animation::WindowAnimation;

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

pub(crate) fn scripting_addition_swap_window_proxy_in(animation_list: &[WindowAnimation]) -> bool {
    let dummy_window_id: u32 = 0;
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &(animation_list.len() as i32).to_ne_bytes()) {
        return false;
    }
    for animation in animation_list {
        if animation.skip.load(Ordering::Relaxed) {
            if !pack(&mut bytes, &mut length, &dummy_window_id.to_ne_bytes()) {
                return false;
            }
        } else {
            if !pack(&mut bytes, &mut length, &animation.window_id.0.to_ne_bytes()) {
                return false;
            }
            if !pack(
                &mut bytes,
                &mut length,
                &animation.proxy.id.load(Ordering::Relaxed).to_ne_bytes(),
            ) {
                return false;
            }
        }
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowSwapProxyIn)
}

pub(crate) fn scripting_addition_swap_window_proxy_out(animation_list: &[WindowAnimation]) -> bool {
    let dummy_window_id: u32 = 0;
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &(animation_list.len() as i32).to_ne_bytes()) {
        return false;
    }
    for animation in animation_list {
        if animation.skip.load(Ordering::Relaxed) {
            if !pack(&mut bytes, &mut length, &dummy_window_id.to_ne_bytes()) {
                return false;
            }
        } else {
            if !pack(&mut bytes, &mut length, &animation.window_id.0.to_ne_bytes()) {
                return false;
            }
            if !pack(
                &mut bytes,
                &mut length,
                &animation.proxy.id.load(Ordering::Relaxed).to_ne_bytes(),
            ) {
                return false;
            }
        }
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
