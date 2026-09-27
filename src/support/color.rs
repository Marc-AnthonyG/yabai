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
