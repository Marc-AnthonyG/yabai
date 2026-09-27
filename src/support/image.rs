#![allow(deprecated)]

use core::ffi::c_void;

use crate::ffi::core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use crate::ffi::core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBitmapInfo, CGColorSpaceCreateDeviceRGB,
    CGContextDrawImage, CGImage, CGImageAlphaInfo, CGImageGetHeight, CGImageGetWidth,
};

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
