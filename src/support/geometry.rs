use crate::ffi::core_foundation::{CGPoint, CGRect};
use crate::ffi::core_graphics::{CGRectGetHeight, CGRectGetWidth};

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
