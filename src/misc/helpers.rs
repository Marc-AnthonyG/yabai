#![allow(deprecated)]

use core::ffi::{c_char, c_void};

use crate::ffi::core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use crate::ffi::core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapInfo, CGColorSpaceCreateDeviceRGB,
    CGContextDrawImage, CGImage, CGImageAlphaInfo, CGImageGetHeight, CGImageGetWidth,
    CGRectGetHeight, CGRectGetWidth,
};

macro_rules! animation_easing_type_list {
    ($animation_easing_type_entry:ident) => {
        $animation_easing_type_entry! {
            (EaseInSine, ease_in_sine, 0),
            (EaseOutSine, ease_out_sine, 1),
            (EaseInOutSine, ease_in_out_sine, 2),
            (EaseInQuad, ease_in_quad, 3),
            (EaseOutQuad, ease_out_quad, 4),
            (EaseInOutQuad, ease_in_out_quad, 5),
            (EaseInCubic, ease_in_cubic, 6),
            (EaseOutCubic, ease_out_cubic, 7),
            (EaseInOutCubic, ease_in_out_cubic, 8),
            (EaseInQuart, ease_in_quart, 9),
            (EaseOutQuart, ease_out_quart, 10),
            (EaseInOutQuart, ease_in_out_quart, 11),
            (EaseInQuint, ease_in_quint, 12),
            (EaseOutQuint, ease_out_quint, 13),
            (EaseInOutQuint, ease_in_out_quint, 14),
            (EaseInExpo, ease_in_expo, 15),
            (EaseOutExpo, ease_out_expo, 16),
            (EaseInOutExpo, ease_in_out_expo, 17),
            (EaseInCirc, ease_in_circ, 18),
            (EaseOutCirc, ease_out_circ, 19),
            (EaseInOutCirc, ease_in_out_circ, 20),
        }
    };
}

macro_rules! define_animation_easing_type {
    ($(($variant:ident, $function:ident, $value:literal)),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(usize)]
        pub enum AnimationEasingType {
            $($variant = $value),*
        }

        pub static ANIMATION_EASING_TYPE_STR: [&str; EASING_TYPE_COUNT] =
            [$(stringify!($function)),*];

        impl AnimationEasingType {
            pub fn apply(self, interpolant: f32) -> f32 {
                match self {
                    $(AnimationEasingType::$variant => $function(interpolant)),*
                }
            }

            pub fn from_index(index: usize) -> Option<AnimationEasingType> {
                match index {
                    $($value => Some(AnimationEasingType::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

animation_easing_type_list!(define_animation_easing_type);

pub const EASING_TYPE_COUNT: usize = 21;

pub fn ease_in_sine(interpolant: f32) -> f32 {
    1.0 - (((interpolant as f64 * std::f64::consts::PI) / 2.0) as f32).cos()
}

pub fn ease_out_sine(interpolant: f32) -> f32 {
    (((interpolant as f64 * std::f64::consts::PI) / 2.0) as f32).sin()
}

pub fn ease_in_out_sine(interpolant: f32) -> f32 {
    -(((std::f64::consts::PI * interpolant as f64) as f32).cos() - 1.0) / 2.0
}

pub fn ease_in_quad(interpolant: f32) -> f32 {
    interpolant * interpolant
}

pub fn ease_out_quad(interpolant: f32) -> f32 {
    1.0 - (1.0 - interpolant) * (1.0 - interpolant)
}

pub fn ease_in_out_quad(interpolant: f32) -> f32 {
    if interpolant < 0.5 {
        2.0 * interpolant * interpolant
    } else {
        1.0 - (-2.0 * interpolant + 2.0).powf(2.0) / 2.0
    }
}

pub fn ease_in_cubic(interpolant: f32) -> f32 {
    interpolant * interpolant * interpolant
}

pub fn ease_out_cubic(interpolant: f32) -> f32 {
    1.0 - (1.0 - interpolant).powf(3.0)
}

pub fn ease_in_out_cubic(interpolant: f32) -> f32 {
    if interpolant < 0.5 {
        4.0 * interpolant * interpolant * interpolant
    } else {
        1.0 - (-2.0 * interpolant + 2.0).powf(3.0) / 2.0
    }
}

pub fn ease_in_quart(interpolant: f32) -> f32 {
    interpolant * interpolant * interpolant * interpolant
}

pub fn ease_out_quart(interpolant: f32) -> f32 {
    1.0 - (1.0 - interpolant).powf(4.0)
}

pub fn ease_in_out_quart(interpolant: f32) -> f32 {
    if interpolant < 0.5 {
        8.0 * interpolant * interpolant * interpolant * interpolant
    } else {
        1.0 - (-2.0 * interpolant + 2.0).powf(4.0) / 2.0
    }
}

pub fn ease_in_quint(interpolant: f32) -> f32 {
    interpolant * interpolant * interpolant * interpolant * interpolant
}

pub fn ease_out_quint(interpolant: f32) -> f32 {
    1.0 - (1.0 - interpolant).powf(5.0)
}

pub fn ease_in_out_quint(interpolant: f32) -> f32 {
    if interpolant < 0.5 {
        16.0 * interpolant * interpolant * interpolant * interpolant * interpolant
    } else {
        1.0 - (-2.0 * interpolant + 2.0).powf(5.0) / 2.0
    }
}

pub fn ease_in_expo(interpolant: f32) -> f32 {
    if interpolant == 0.0 {
        0.0
    } else {
        2.0f32.powf(10.0 * interpolant - 10.0)
    }
}

pub fn ease_out_expo(interpolant: f32) -> f32 {
    if interpolant == 1.0 {
        1.0
    } else {
        1.0 - 2.0f32.powf(-10.0 * interpolant)
    }
}

pub fn ease_in_out_expo(interpolant: f32) -> f32 {
    if interpolant == 0.0 {
        0.0
    } else if interpolant == 1.0 {
        1.0
    } else if interpolant < 0.5 {
        2.0f32.powf(20.0 * interpolant - 10.0) / 2.0
    } else {
        (2.0 - 2.0f32.powf(-20.0 * interpolant + 10.0)) / 2.0
    }
}

pub fn ease_in_circ(interpolant: f32) -> f32 {
    1.0 - (1.0 - interpolant.powf(2.0)).sqrt()
}

pub fn ease_out_circ(interpolant: f32) -> f32 {
    (1.0 - (interpolant - 1.0).powf(2.0)).sqrt()
}

pub fn ease_in_out_circ(interpolant: f32) -> f32 {
    if interpolant < 0.5 {
        (1.0 - (1.0 - (2.0 * interpolant).powf(2.0)).sqrt()) / 2.0
    } else {
        ((1.0 - (-2.0 * interpolant + 2.0).powf(2.0)).sqrt() + 1.0) / 2.0
    }
}

#[derive(Clone, Copy)]
pub struct RgbaColor {
    pub packed: u32,
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

pub static BOOL_STR: [&str; 2] = ["off", "on"];

pub static LAYER_STR: [Option<&str>; 6] = [
    Some("auto"),
    None,
    None,
    Some("below"),
    Some("normal"),
    Some("above"),
];

pub fn socket_open(socket_file_descriptor: &mut i32) -> bool {
    *socket_file_descriptor = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    *socket_file_descriptor != -1
}

pub fn socket_connect(socket_file_descriptor: i32, socket_path: &str) -> bool {
    let mut socket_address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    socket_address.sun_family = libc::AF_UNIX as libc::sa_family_t;

    let socket_path_bytes = socket_path.as_bytes();
    let writable_length = socket_address.sun_path.len() - 1;
    let copied_length = socket_path_bytes.len().min(writable_length);
    for index in 0..copied_length {
        socket_address.sun_path[index] = socket_path_bytes[index] as c_char;
    }

    unsafe {
        libc::connect(
            socket_file_descriptor,
            (&socket_address as *const libc::sockaddr_un).cast::<libc::sockaddr>(),
            std::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        ) != -1
    }
}

pub fn socket_close(socket_file_descriptor: i32) {
    unsafe {
        libc::shutdown(socket_file_descriptor, libc::SHUT_RDWR);
        libc::close(socket_file_descriptor);
    }
}

pub fn json_optional_bool(value: i32) -> &'static str {
    if value == 0 {
        return "null";
    }
    if value == 1 {
        return "true";
    }

    "false"
}

pub fn json_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
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

pub fn is_root() -> bool {
    unsafe { libc::getuid() == 0 || libc::geteuid() == 0 }
}

pub fn string_equals(first: Option<&str>, second: Option<&str>) -> bool {
    matches!((first, second), (Some(first), Some(second)) if first == second)
}

pub fn ts_string_escape(string: &str) -> Option<String> {
    let mut number_of_replacements = 0;

    for cursor in string.chars() {
        if (cursor == '"')
            || (cursor == '\\')
            || (cursor == '\u{8}')
            || (cursor == '\u{c}')
            || (cursor == '\n')
            || (cursor == '\r')
            || (cursor == '\t')
        {
            number_of_replacements += 1;
        } else if cursor <= '\u{1f}' {
            number_of_replacements += 5;
        }
    }

    if number_of_replacements == 0 {
        return None;
    }

    let size_in_bytes = string.len() + number_of_replacements;
    let mut destination = String::with_capacity(size_in_bytes);

    for cursor in string.chars() {
        if cursor == '"' {
            destination.push_str("\\\"");
        } else if cursor == '\\' {
            destination.push_str("\\\\");
        } else if cursor == '\u{8}' {
            destination.push_str("\\b");
        } else if cursor == '\u{c}' {
            destination.push_str("\\f");
        } else if cursor == '\n' {
            destination.push_str("\\n");
        } else if cursor == '\r' {
            destination.push_str("\\r");
        } else if cursor == '\t' {
            destination.push_str("\\t");
        } else if cursor <= '\u{1f}' {
            destination.push_str(&format!("\\u{:04x}", cursor as u32));
        } else {
            destination.push(cursor);
        }
    }

    Some(destination)
}

pub fn string_copy(string: &str) -> String {
    string.to_owned()
}

pub fn directory_exists(filename: &str) -> bool {
    match std::fs::metadata(filename) {
        Ok(buffer) => buffer.is_dir(),
        Err(_) => false,
    }
}

pub fn file_exists(filename: &str) -> bool {
    match std::fs::metadata(filename) {
        Ok(buffer) => !buffer.is_dir(),
        Err(_) => false,
    }
}

pub fn file_can_execute(filename: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(filename) {
        Ok(buffer) => (buffer.permissions().mode() & libc::S_IXUSR as u32) != 0,
        Err(_) => false,
    }
}

pub fn get_config_file(filename: &str) -> Option<String> {
    if let Some(xdg_home) = std::env::var_os("XDG_CONFIG_HOME")
        && !xdg_home.is_empty()
    {
        let buffer = format!("{}/yabai/{}", xdg_home.to_string_lossy(), filename);
        if file_exists(&buffer) {
            return Some(buffer);
        }
    }

    let home = std::env::var_os("HOME")?;

    let buffer = format!("{}/.config/yabai/{}", home.to_string_lossy(), filename);
    if file_exists(&buffer) {
        return Some(buffer);
    }

    let buffer = format!("{}/.{}", home.to_string_lossy(), filename);
    file_exists(&buffer).then_some(buffer)
}

pub fn exec_config_file(config_file: String) {
    let config_file = if config_file.is_empty() {
        match get_config_file("yabairc") {
            Some(config_file) => config_file,
            None => {
                crate::warn!("yabai: could not locate config file..\n");
                crate::notify!("configuration", "could not locate config file..");
                return;
            }
        }
    } else {
        config_file
    };

    if !file_exists(&config_file) {
        crate::warn!(
            "yabai: configuration file '{}' does not exist..\n",
            config_file
        );
        crate::notify!("configuration", "file '{}' does not exist..", config_file);
        return;
    }

    let config_file_argument = std::ffi::CString::new(config_file.as_str()).unwrap();
    let exec: [*const c_char; 5] = if file_can_execute(&config_file) {
        [
            c"/usr/bin/env".as_ptr(),
            c"sh".as_ptr(),
            c"-c".as_ptr(),
            config_file_argument.as_ptr(),
            std::ptr::null(),
        ]
    } else {
        [
            c"/usr/bin/env".as_ptr(),
            c"sh".as_ptr(),
            config_file_argument.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
        ]
    };

    let process_id = unsafe { libc::fork() };
    if process_id == 0 {
        unsafe { libc::_exit(libc::execvp(exec[0], exec.as_ptr())) };
    } else if process_id == -1 {
        crate::warn!("yabai: failed to load config file '{}'\n", config_file);
        crate::notify!("configuration", "failed to load file '{}'", config_file);
    }
}

pub fn cgrect_clamp_x_radius(frame: CGRect, radius: f32) -> f32 {
    let mut radius = radius;
    if (radius * 2.0) as f64 > CGRectGetWidth(frame) {
        radius = (CGRectGetWidth(frame) / 2.0) as f32;
    }
    radius
}

pub fn cgrect_clamp_y_radius(frame: CGRect, radius: f32) -> f32 {
    let mut radius = radius;
    if (radius * 2.0) as f64 > CGRectGetHeight(frame) {
        radius = (CGRectGetHeight(frame) / 2.0) as f32;
    }
    radius
}

pub fn cgrect_contains_point(rect: CGRect, point: CGPoint) -> bool {
    point.x >= rect.origin.x
        && point.x <= rect.origin.x + rect.size.width
        && point.y >= rect.origin.y
        && point.y <= rect.origin.y + rect.size.height
}

pub fn triangle_contains_point(triangle: &[CGPoint; 3], point: CGPoint) -> bool {
    let first_edge_cross = ((point.x - triangle[0].x) * (triangle[2].y - triangle[0].y)
        - (triangle[2].x - triangle[0].x) * (point.y - triangle[0].y))
        as f32;
    let second_edge_cross = ((point.x - triangle[1].x) * (triangle[0].y - triangle[1].y)
        - (triangle[0].x - triangle[1].x) * (point.y - triangle[1].y))
        as f32;
    let third_edge_cross = ((point.x - triangle[2].x) * (triangle[1].y - triangle[2].y)
        - (triangle[1].x - triangle[2].x) * (point.y - triangle[2].y))
        as f32;

    (first_edge_cross > 0.0 && second_edge_cross > 0.0 && third_edge_cross > 0.0)
        || (first_edge_cross < 0.0 && second_edge_cross < 0.0 && third_edge_cross < 0.0)
}

pub fn clampf_range(value: f32, minimum: f32, maximum: f32) -> f32 {
    if value < minimum {
        return minimum;
    }
    if value > maximum {
        return maximum;
    }
    value
}

pub fn cgimage_restore_alpha(image: &CGImage) -> Option<CFRetained<CGImage>> {
    let width = CGImageGetWidth(Some(image));
    let height = CGImageGetHeight(Some(image));
    let pitch = width * 4;
    let mut data: Vec<u8> = vec![0; height * pitch];

    let color_space = CGColorSpaceCreateDeviceRGB();
    let context = unsafe {
        CGBitmapContextCreate(
            data.as_mut_ptr().cast::<c_void>(),
            width,
            height,
            8,
            pitch,
            color_space.as_deref(),
            CGBitmapInfo::ByteOrder32Big.0 | CGImageAlphaInfo::PremultipliedLast.0,
        )
    };
    drop(color_space);
    CGContextDrawImage(
        context.as_deref(),
        CGRect::new(
            CGPoint::new(0.0, 0.0),
            CGSize::new(width as f64, height as f64),
        ),
        Some(image),
    );

    let pixel_count = height * width;
    let whole_group_count = pixel_count / 4;
    for group_index in 0..whole_group_count {
        unsafe {
            restore_alpha_four_pixels(data.as_mut_ptr().add(group_index * 16).cast::<u32>());
        }
    }

    let remainder_pixel_count = pixel_count - whole_group_count * 4;
    if remainder_pixel_count != 0 {
        let mut scratch: [u32; 4] = [0; 4];
        let tail_offset = whole_group_count * 16;
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr().add(tail_offset),
                scratch.as_mut_ptr().cast::<u8>(),
                remainder_pixel_count * 4,
            );
            restore_alpha_four_pixels(scratch.as_mut_ptr());
            std::ptr::copy_nonoverlapping(
                scratch.as_ptr().cast::<u8>(),
                data.as_mut_ptr().add(tail_offset),
                remainder_pixel_count * 4,
            );
        }
    }

    let result = CGBitmapContextCreateImage(context.as_deref());
    drop(context);
    drop(data);

    result
}

#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn restore_alpha_four_pixels(pixel: *mut u32) {
    use std::arch::x86_64::{
        __m128i, _mm_and_si128, _mm_andnot_si128, _mm_castps_si128, _mm_cmpgt_ps, _mm_cvtepi32_ps,
        _mm_cvtps_epi32, _mm_div_ps, _mm_loadu_si128, _mm_mul_ps, _mm_or_si128, _mm_set1_epi32,
        _mm_set1_ps, _mm_slli_epi32, _mm_srli_epi32, _mm_storeu_si128,
    };

    unsafe {
        let inv255 = _mm_set1_ps(1.0 / 255.0);
        let one255 = _mm_set1_ps(255.0);
        let zero = _mm_set1_ps(0.0);
        let mask_ff = _mm_set1_epi32(0xff);

        let source = _mm_loadu_si128(pixel.cast::<__m128i>());
        let mut red = _mm_cvtepi32_ps(_mm_and_si128(source, mask_ff));
        let mut green = _mm_cvtepi32_ps(_mm_and_si128(_mm_srli_epi32::<8>(source), mask_ff));
        let mut blue = _mm_cvtepi32_ps(_mm_and_si128(_mm_srli_epi32::<16>(source), mask_ff));
        let mut alpha = _mm_cvtepi32_ps(_mm_and_si128(_mm_srli_epi32::<24>(source), mask_ff));
        let mask = _mm_castps_si128(_mm_cmpgt_ps(alpha, zero));

        red = _mm_mul_ps(one255, _mm_div_ps(red, alpha));
        green = _mm_mul_ps(one255, _mm_div_ps(green, alpha));
        blue = _mm_mul_ps(one255, _mm_div_ps(blue, alpha));

        alpha = one255;

        red = _mm_mul_ps(inv255, _mm_mul_ps(red, alpha));
        green = _mm_mul_ps(inv255, _mm_mul_ps(green, alpha));
        blue = _mm_mul_ps(inv255, _mm_mul_ps(blue, alpha));

        let source_red = _mm_cvtps_epi32(red);
        let source_green = _mm_slli_epi32::<8>(_mm_cvtps_epi32(green));
        let source_blue = _mm_slli_epi32::<16>(_mm_cvtps_epi32(blue));
        let source_alpha = _mm_slli_epi32::<24>(_mm_cvtps_epi32(alpha));

        let color = _mm_or_si128(
            _mm_or_si128(_mm_or_si128(source_red, source_green), source_blue),
            source_alpha,
        );
        let masked_color = _mm_or_si128(
            _mm_and_si128(mask, color),
            _mm_andnot_si128(mask, source),
        );
        _mm_storeu_si128(pixel.cast::<__m128i>(), masked_color);
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn restore_alpha_four_pixels(pixel: *mut u32) {
    use std::arch::aarch64::{
        vandq_s32, vbicq_s32, vcgtq_f32, vcvtnq_s32_f32, vcvtq_f32_s32, vdivq_f32, vdupq_n_f32,
        vdupq_n_s32, vld1q_s32, vmulq_f32, vorrq_s32, vreinterpretq_s32_u32, vreinterpretq_u32_s32,
        vshlq_s32, vshlq_u32, vst1q_s32,
    };

    unsafe {
        let inv255 = vdupq_n_f32(1.0 / 255.0);
        let one255 = vdupq_n_f32(255.0);
        let zero = vdupq_n_f32(0.0);
        let mask_ff = vdupq_n_s32(0xff);

        let source = vld1q_s32(pixel.cast::<i32>());
        let mut red = vcvtq_f32_s32(vandq_s32(source, mask_ff));
        let mut green = vcvtq_f32_s32(vandq_s32(
            vreinterpretq_s32_u32(vshlq_u32(vreinterpretq_u32_s32(source), vdupq_n_s32(-8))),
            mask_ff,
        ));
        let mut blue = vcvtq_f32_s32(vandq_s32(
            vreinterpretq_s32_u32(vshlq_u32(vreinterpretq_u32_s32(source), vdupq_n_s32(-16))),
            mask_ff,
        ));
        let mut alpha = vcvtq_f32_s32(vandq_s32(
            vreinterpretq_s32_u32(vshlq_u32(vreinterpretq_u32_s32(source), vdupq_n_s32(-24))),
            mask_ff,
        ));
        let mask = vreinterpretq_s32_u32(vcgtq_f32(alpha, zero));

        red = vmulq_f32(one255, vdivq_f32(red, alpha));
        green = vmulq_f32(one255, vdivq_f32(green, alpha));
        blue = vmulq_f32(one255, vdivq_f32(blue, alpha));

        alpha = one255;

        red = vmulq_f32(inv255, vmulq_f32(red, alpha));
        green = vmulq_f32(inv255, vmulq_f32(green, alpha));
        blue = vmulq_f32(inv255, vmulq_f32(blue, alpha));

        let source_red = vcvtnq_s32_f32(red);
        let source_green = vshlq_s32(vcvtnq_s32_f32(green), vdupq_n_s32(8));
        let source_blue = vshlq_s32(vcvtnq_s32_f32(blue), vdupq_n_s32(16));
        let source_alpha = vshlq_s32(vcvtnq_s32_f32(alpha), vdupq_n_s32(24));

        let color = vorrq_s32(
            vorrq_s32(vorrq_s32(source_red, source_green), source_blue),
            source_alpha,
        );
        let masked_color = vorrq_s32(vandq_s32(color, mask), vbicq_s32(source, mask));
        vst1q_s32(pixel.cast::<i32>(), masked_color);
    }
}
