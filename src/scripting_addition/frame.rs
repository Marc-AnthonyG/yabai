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
    write_length_header_and_opcode_into_frame(bytes, length, opcode);
    scripting_addition_send_bytes(&bytes[..length as usize])
}

fn write_length_header_and_opcode_into_frame(bytes: &mut [u8], length: i16, opcode: SaOpcode) {
    bytes[..size_of::<i16>()]
        .copy_from_slice(&((length as usize - size_of::<i16>()) as i16).to_ne_bytes());
    bytes[size_of::<i16>()] = opcode as u8;
}

#[cfg(test)]
mod tests {
    use super::{SA_SOCKET_BUFF_LEN, SaOpcode, pack, write_length_header_and_opcode_into_frame};

    const LENGTH_OF_THE_HEADER_AND_OPCODE: i16 = 1 + 2;

    fn pack_every_field(bytes: &mut [u8], length: &mut i16, fields: &[&[u8]]) {
        for field in fields {
            assert!(
                pack(bytes, length, field),
                "a field of {} bytes should fit",
                field.len()
            );
        }
    }

    fn frame_with_fields(fields: &[&[u8]], opcode: SaOpcode) -> Vec<u8> {
        let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
        let mut length = LENGTH_OF_THE_HEADER_AND_OPCODE;
        pack_every_field(&mut bytes, &mut length, fields);
        write_length_header_and_opcode_into_frame(&mut bytes, length, opcode);
        bytes[..length as usize].to_vec()
    }

    #[test]
    fn a_space_move_frame_matches_the_bytes_the_c_macros_write() {
        let frame = frame_with_fields(
            &[
                &0x1122334455667788u64.to_ne_bytes(),
                &2u64.to_ne_bytes(),
                &3u64.to_ne_bytes(),
                &[true as u8],
            ],
            SaOpcode::SpaceMove,
        );

        assert_eq!(
            frame,
            [
                0x1a, 0x00, 0x05, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x02, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            ]
        );
    }

    #[test]
    fn a_window_scale_frame_packs_its_floats_without_padding_as_the_c_macros_do() {
        let frame = frame_with_fields(
            &[
                &0xABCDu32.to_ne_bytes(),
                &1.5f32.to_ne_bytes(),
                &(-2.0f32).to_ne_bytes(),
                &100.25f32.to_ne_bytes(),
                &50.0f32.to_ne_bytes(),
            ],
            SaOpcode::WindowScale,
        );

        assert_eq!(
            frame,
            [
                0x15, 0x00, 0x0d, 0xcd, 0xab, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x3f, 0x00, 0x00, 0x00,
                0xc0, 0x00, 0x80, 0xc8, 0x42, 0x00, 0x00, 0x48, 0x42,
            ]
        );
    }

    #[test]
    fn a_space_focus_frame_matches_the_bytes_the_c_macros_write() {
        let frame = frame_with_fields(
            &[&0x0102030405060708u64.to_ne_bytes()],
            SaOpcode::SpaceFocus,
        );

        assert_eq!(
            frame,
            [
                0x09, 0x00, 0x02, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01
            ]
        );
    }

    #[test]
    fn the_header_holds_the_length_without_itself_as_a_native_order_i16_followed_by_the_opcode() {
        let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];

        write_length_header_and_opcode_into_frame(&mut bytes, 0x0203, SaOpcode::WindowOrderIn);

        assert_eq!(bytes[..2], 0x0201i16.to_ne_bytes());
        assert_eq!(bytes[2], 0x11);
    }

    #[test]
    fn pack_advances_the_length_by_the_size_of_each_field() {
        let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
        let mut length = LENGTH_OF_THE_HEADER_AND_OPCODE;

        pack_every_field(
            &mut bytes,
            &mut length,
            &[&[0xaa], &[0xbb, 0xcc], &[0xdd; 8]],
        );

        assert_eq!(length, 3 + 1 + 2 + 8);
        assert_eq!(
            bytes[3..14],
            [
                0xaa, 0xbb, 0xcc, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd
            ]
        );
    }

    #[test]
    fn pack_accepts_a_field_that_ends_exactly_at_the_end_of_the_4096_byte_buffer() {
        let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
        let mut length: i16 = 4088;

        assert!(pack(&mut bytes, &mut length, &u64::MAX.to_ne_bytes()));

        assert_eq!(length, 4096);
        assert_eq!(bytes[4088..], [0xff; 8]);
    }

    #[test]
    fn pack_refuses_a_field_that_would_overflow_the_buffer_and_leaves_length_and_bytes_alone() {
        let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
        let mut length: i16 = 4090;

        assert!(!pack(&mut bytes, &mut length, &u64::MAX.to_ne_bytes()));

        assert_eq!(length, 4090);
        assert!(bytes.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn pack_refuses_even_one_byte_once_the_buffer_is_full() {
        let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
        let mut length = LENGTH_OF_THE_HEADER_AND_OPCODE;
        pack_every_field(&mut bytes, &mut length, &[&[0x01; SA_SOCKET_BUFF_LEN - 3]]);

        assert!(!pack(&mut bytes, &mut length, &[0x02]));

        assert_eq!(length, 4096);
    }

    #[test]
    fn the_frame_buffer_is_4096_bytes_as_in_the_payload_header() {
        assert_eq!(SA_SOCKET_BUFF_LEN, 4096);
    }
}
