use crate::ffi::core_foundation::{CGPoint, CGRect};

pub fn is_point_inside_rectangle_including_its_edges(rect: CGRect, point: CGPoint) -> bool {
    point.x >= rect.origin.x
        && point.x <= rect.origin.x + rect.size.width
        && point.y >= rect.origin.y
        && point.y <= rect.origin.y + rect.size.height
}

pub fn is_point_strictly_inside_triangle(triangle: &[CGPoint; 3], point: CGPoint) -> bool {
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
