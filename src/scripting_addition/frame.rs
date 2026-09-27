use crate::scripting_addition::client::scripting_addition_send_bytes;

include!(concat!(env!("OUT_DIR"), "/osax_common.rs"));

pub(crate) fn pack(bytes: &mut [u8], length: &mut i16, value: &[u8]) -> bool {
    let offset = *length as usize;
    let Some(destination) = bytes.get_mut(offset..offset + value.len()) else {
        return false;
    };
    destination.copy_from_slice(value);
    *length += value.len() as i16;
    true
}

pub(crate) fn sa_payload_send(bytes: &mut [u8], length: i16, opcode: SaOpcode) -> bool {
    bytes[..size_of::<i16>()]
        .copy_from_slice(&((length as usize - size_of::<i16>()) as i16).to_ne_bytes());
    bytes[size_of::<i16>()] = opcode as u8;
    scripting_addition_send_bytes(&bytes[..length as usize])
}
