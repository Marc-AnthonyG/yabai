#[derive(Clone, Copy)]
pub struct RgbaColor {
    pub packed: u32,
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

pub fn rgba_color_from_hex(color: u32) -> RgbaColor {
    RgbaColor {
        packed: color,
        red: ((color >> 0x10) & 0xff) as f32 / 255.0,
        green: ((color >> 0x08) & 0xff) as f32 / 255.0,
        blue: ((color >> 0x00) & 0xff) as f32 / 255.0,
        alpha: ((color >> 0x18) & 0xff) as f32 / 255.0,
    }
}

#[cfg(test)]
mod tests {
    use super::rgba_color_from_hex;

    fn packed_and_channel_bits_of(color: u32) -> (u32, [u32; 4]) {
        let rgba_color = rgba_color_from_hex(color);
        (
            rgba_color.packed,
            [
                rgba_color.red.to_bits(),
                rgba_color.green.to_bits(),
                rgba_color.blue.to_bits(),
                rgba_color.alpha.to_bits(),
            ],
        )
    }

    #[test]
    fn rgba_color_from_hex_reads_alpha_from_the_top_byte_then_red_green_and_blue() {
        let expected_channel_bits = [
            (0xffffffff, [0x3f800000, 0x3f800000, 0x3f800000, 0x3f800000]),
            (0x00000000, [0x00000000, 0x00000000, 0x00000000, 0x00000000]),
            (0xffff0000, [0x3f800000, 0x00000000, 0x00000000, 0x3f800000]),
            (0x01000000, [0x00000000, 0x00000000, 0x00000000, 0x3b808081]),
        ];

        for (color, expected_bits) in expected_channel_bits {
            assert_eq!(
                packed_and_channel_bits_of(color),
                (color, expected_bits),
                "color {color:#010x}"
            );
        }
    }

    #[test]
    fn rgba_color_from_hex_divides_each_channel_by_255_in_f32_as_the_c_does() {
        let expected_channel_bits = [
            (0xff0f1e2d, [0x3d70f0f1, 0x3df0f0f1, 0x3e34b4b5, 0x3f800000]),
            (0x80abcdef, [0x3f2babac, 0x3f4dcdce, 0x3f6feff0, 0x3f008081]),
            (0x7f336699, [0x3e4ccccd, 0x3ecccccd, 0x3f19999a, 0x3efefeff]),
        ];

        for (color, expected_bits) in expected_channel_bits {
            assert_eq!(
                packed_and_channel_bits_of(color),
                (color, expected_bits),
                "color {color:#010x}"
            );
        }
    }
}
