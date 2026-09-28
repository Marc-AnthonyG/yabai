use crate::scripting_addition::client::send_frame_to_scripting_addition_and_wait_for_acknowledgement;

include!(concat!(env!("OUT_DIR"), "/osax_common.rs"));

const LENGTH_OF_THE_HEADER_AND_OPCODE: usize = size_of::<i16>() + 1;

pub(crate) struct ScriptingAdditionFrame {
    bytes: [u8; SCRIPTING_ADDITION_FRAME_BUFFER_LENGTH],
    length: usize,
    a_field_did_not_fit: bool,
}

impl ScriptingAdditionFrame {
    pub(crate) fn new() -> ScriptingAdditionFrame {
        ScriptingAdditionFrame {
            bytes: [0; SCRIPTING_ADDITION_FRAME_BUFFER_LENGTH],
            length: LENGTH_OF_THE_HEADER_AND_OPCODE,
            a_field_did_not_fit: false,
        }
    }

    pub(crate) fn append(&mut self, field: &[u8]) -> &mut ScriptingAdditionFrame {
        match self.bytes.get_mut(self.length..self.length + field.len()) {
            Some(destination) if !self.a_field_did_not_fit => {
                destination.copy_from_slice(field);
                self.length += field.len();
            }
            _ => self.a_field_did_not_fit = true,
        }
        self
    }

    pub(crate) fn finish_with_opcode(&mut self, opcode: ScriptingAdditionOpcode) -> Option<&[u8]> {
        if self.a_field_did_not_fit {
            return None;
        }
        let length_without_its_own_field = (self.length - size_of::<i16>()) as i16;
        self.bytes[..size_of::<i16>()].copy_from_slice(&length_without_its_own_field.to_ne_bytes());
        self.bytes[size_of::<i16>()] = opcode as u8;
        Some(&self.bytes[..self.length])
    }

    pub(crate) fn send_as(&mut self, opcode: ScriptingAdditionOpcode) -> bool {
        self.finish_with_opcode(opcode)
            .is_some_and(send_frame_to_scripting_addition_and_wait_for_acknowledgement)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SCRIPTING_ADDITION_FRAME_BUFFER_LENGTH, ScriptingAdditionFrame, ScriptingAdditionOpcode,
    };

    fn frame_with_fields(fields: &[&[u8]], opcode: ScriptingAdditionOpcode) -> Option<Vec<u8>> {
        let mut frame = ScriptingAdditionFrame::new();
        for field in fields {
            frame.append(field);
        }
        frame.finish_with_opcode(opcode).map(<[u8]>::to_vec)
    }

    #[test]
    fn a_space_move_frame_matches_the_bytes_the_payload_parses() {
        let frame = frame_with_fields(
            &[
                &0x1122334455667788u64.to_ne_bytes(),
                &2u64.to_ne_bytes(),
                &3u64.to_ne_bytes(),
                &[true as u8],
            ],
            ScriptingAdditionOpcode::SpaceMove,
        );

        assert_eq!(
            frame.unwrap(),
            [
                0x1a, 0x00, 0x05, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x02, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            ]
        );
    }

    #[test]
    fn a_window_scale_frame_packs_its_floats_without_padding() {
        let frame = frame_with_fields(
            &[
                &0xABCDu32.to_ne_bytes(),
                &1.5f32.to_ne_bytes(),
                &(-2.0f32).to_ne_bytes(),
                &100.25f32.to_ne_bytes(),
                &50.0f32.to_ne_bytes(),
            ],
            ScriptingAdditionOpcode::WindowScale,
        );

        assert_eq!(
            frame.unwrap(),
            [
                0x15, 0x00, 0x0d, 0xcd, 0xab, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x3f, 0x00, 0x00, 0x00,
                0xc0, 0x00, 0x80, 0xc8, 0x42, 0x00, 0x00, 0x48, 0x42,
            ]
        );
    }

    #[test]
    fn a_space_focus_frame_matches_the_bytes_the_payload_parses() {
        let frame = frame_with_fields(
            &[&0x0102030405060708u64.to_ne_bytes()],
            ScriptingAdditionOpcode::SpaceFocus,
        );

        assert_eq!(
            frame.unwrap(),
            [
                0x09, 0x00, 0x02, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01
            ]
        );
    }

    #[test]
    fn a_frame_without_fields_holds_only_its_length_and_opcode() {
        assert_eq!(
            frame_with_fields(&[], ScriptingAdditionOpcode::WindowOrderIn).unwrap(),
            [0x01, 0x00, 0x11]
        );
    }

    #[test]
    fn a_field_that_ends_exactly_at_the_end_of_the_buffer_fits() {
        let filler = [0x01; SCRIPTING_ADDITION_FRAME_BUFFER_LENGTH - 3 - 8];
        let frame = frame_with_fields(
            &[&filler, &u64::MAX.to_ne_bytes()],
            ScriptingAdditionOpcode::SpaceFocus,
        )
        .unwrap();

        assert_eq!(frame.len(), SCRIPTING_ADDITION_FRAME_BUFFER_LENGTH);
        assert_eq!(
            frame[SCRIPTING_ADDITION_FRAME_BUFFER_LENGTH - 8..],
            [0xff; 8]
        );
    }

    #[test]
    fn a_field_that_would_overflow_the_buffer_fails_the_whole_frame_even_if_a_later_one_fits() {
        let filler = [0x01; SCRIPTING_ADDITION_FRAME_BUFFER_LENGTH - 3 - 6];

        assert!(
            frame_with_fields(
                &[&filler, &u64::MAX.to_ne_bytes(), &[0x02]],
                ScriptingAdditionOpcode::SpaceFocus,
            )
            .is_none()
        );
    }

    #[test]
    fn the_frame_buffer_is_4096_bytes_as_in_the_payload_header() {
        assert_eq!(SCRIPTING_ADDITION_FRAME_BUFFER_LENGTH, 4096);
    }
}
