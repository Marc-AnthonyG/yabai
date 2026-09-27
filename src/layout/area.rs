use crate::ffi::core_foundation::{CGPoint, CGRect};
use crate::layout::tree::WindowNodeSplit;
use crate::support::direction::{DIR_EAST, DIR_NORTH, DIR_SOUTH, DIR_WEST};

#[derive(Clone, Copy, Default)]
pub(crate) struct Area {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

pub(crate) fn area_from_cgrect(rect: CGRect) -> Area {
    Area {
        x: rect.origin.x as f32,
        y: rect.origin.y as f32,
        width: rect.size.width as f32,
        height: rect.size.height as f32,
    }
}

pub(crate) fn area_max_point(area: Area) -> CGPoint {
    CGPoint {
        x: (area.x + area.width - 1.0f32) as f64,
        y: (area.y + area.height - 1.0f32) as f64,
    }
}

pub(crate) fn area_make_pair(
    split: WindowNodeSplit,
    gap: i32,
    ratio: f32,
    parent_area: Area,
) -> (Area, Area) {
    if split == WindowNodeSplit::Y {
        let mut left_area = parent_area;
        let mut right_area = parent_area;

        let left_width = (parent_area.width - gap as f32) * ratio;
        let right_width = (parent_area.width - gap as f32) * (1.0f32 - ratio);

        left_area.width = left_width as i32 as f32;
        right_area.width = right_width as i32 as f32;
        right_area.x += ((left_width + 0.5f32) as i32 + gap) as f32;

        (left_area, right_area)
    } else {
        let mut left_area = parent_area;
        let mut right_area = parent_area;

        let left_width = (parent_area.height - gap as f32) * ratio;
        let right_width = (parent_area.height - gap as f32) * (1.0f32 - ratio);

        left_area.height = left_width as i32 as f32;
        right_area.height = right_width as i32 as f32;
        right_area.y += ((left_width + 0.5f32) as i32 + gap) as f32;

        (left_area, right_area)
    }
}

pub(crate) fn area_is_in_direction(
    first_area: &Area,
    first_area_max_point: CGPoint,
    second_area: &Area,
    second_area_max_point: CGPoint,
    direction: i32,
) -> bool {
    if direction == DIR_NORTH && first_area_max_point.y <= second_area.y as f64 {
        return false;
    }
    if direction == DIR_EAST && second_area_max_point.x <= first_area.x as f64 {
        return false;
    }
    if direction == DIR_SOUTH && second_area_max_point.y <= first_area.y as f64 {
        return false;
    }
    if direction == DIR_WEST && first_area_max_point.x <= second_area.x as f64 {
        return false;
    }

    if direction == DIR_NORTH || direction == DIR_SOUTH {
        return (second_area_max_point.x > first_area.x as f64
            && second_area_max_point.x <= first_area_max_point.x)
            || (second_area.x < first_area.x
                && second_area_max_point.x > first_area_max_point.x)
            || (second_area.x >= first_area.x
                && (second_area.x as f64) < first_area_max_point.x);
    }

    if direction == DIR_EAST || direction == DIR_WEST {
        return (second_area_max_point.y > first_area.y as f64
            && second_area_max_point.y <= first_area_max_point.y)
            || (second_area.y < first_area.y
                && second_area_max_point.y > first_area_max_point.y)
            || (second_area.y >= first_area.y
                && (second_area.y as f64) < first_area_max_point.y);
    }

    false
}

pub(crate) fn area_distance_in_direction(
    first_area: &Area,
    first_area_max_point: CGPoint,
    second_area: &Area,
    second_area_max_point: CGPoint,
    direction: i32,
) -> i32 {
    match direction {
        DIR_NORTH => {
            return (if second_area_max_point.y > first_area.y as f64 {
                second_area_max_point.y - first_area.y as f64
            } else {
                first_area.y as f64 - second_area_max_point.y
            }) as i32;
        }
        DIR_EAST => {
            return (if (second_area.x as f64) < first_area_max_point.x {
                first_area_max_point.x - second_area.x as f64
            } else {
                second_area.x as f64 - first_area_max_point.x
            }) as i32;
        }
        DIR_SOUTH => {
            return (if (second_area.y as f64) < first_area_max_point.y {
                first_area_max_point.y - second_area.y as f64
            } else {
                second_area.y as f64 - first_area_max_point.y
            }) as i32;
        }
        DIR_WEST => {
            return (if second_area_max_point.x > first_area.x as f64 {
                second_area_max_point.x - first_area.x as f64
            } else {
                first_area.x as f64 - second_area_max_point.x
            }) as i32;
        }
        _ => {}
    }

    i32::MAX
}

pub(crate) fn ax_diff(first: f64, second: f64) -> bool {
    let difference = first - second;
    let absolute = if difference < 0.0f64 {
        difference * -1.0f64
    } else {
        difference
    };
    absolute >= 1.5f32 as f64
}

#[cfg(test)]
mod tests {
    use super::{
        Area, area_distance_in_direction, area_is_in_direction, area_make_pair, area_max_point,
    };
    use crate::ffi::core_foundation::CGPoint;
    use crate::layout::tree::WindowNodeSplit;
    use crate::support::direction::{DIR_EAST, DIR_WEST};

    struct TestArea {
        area: Area,
        area_max: CGPoint,
    }

    fn init_test_display_list() -> [TestArea; 3] {
        let mut display_list: [TestArea; 3] = std::array::from_fn(|_| TestArea {
            area: Area::default(),
            area_max: CGPoint { x: 0.0, y: 0.0 },
        });

        display_list[0].area.x = 0.0;
        display_list[0].area.y = 0.0;
        display_list[0].area.width = 2560.0;
        display_list[0].area.height = 1440.0;
        display_list[0].area_max = area_max_point(display_list[0].area);

        display_list[1].area.x = -1728.0;
        display_list[1].area.y = 0.0;
        display_list[1].area.width = 1728.0;
        display_list[1].area.height = 1117.0;
        display_list[1].area_max = area_max_point(display_list[1].area);

        display_list[2].area.x = 2560.0;
        display_list[2].area.y = 0.0;
        display_list[2].area.width = 1920.0;
        display_list[2].area.height = 1080.0;
        display_list[2].area_max = area_max_point(display_list[2].area);

        display_list
    }

    #[test]
    fn test_display_area_is_in_direction() {
        let display_list = init_test_display_list();

        let t1 = area_is_in_direction(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[1].area,
            display_list[1].area_max,
            DIR_WEST,
        );
        assert_eq!(t1, true);

        let t2 = area_is_in_direction(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[1].area,
            display_list[1].area_max,
            DIR_EAST,
        );
        assert_eq!(t2, false);

        let t3 = area_is_in_direction(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[2].area,
            display_list[2].area_max,
            DIR_WEST,
        );
        assert_eq!(t3, false);

        let t4 = area_is_in_direction(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[2].area,
            display_list[2].area_max,
            DIR_EAST,
        );
        assert_eq!(t4, true);
    }

    fn closest_display_in_direction(
        display_list: &[TestArea],
        display_count: i32,
        source: i32,
        direction: i32,
    ) -> i32 {
        let mut best_index = -1;
        let mut best_distance = i32::MAX;

        for index in 0..display_count {
            if index == source {
                continue;
            }

            let source_display = &display_list[source as usize];
            let candidate_display = &display_list[index as usize];
            if area_is_in_direction(
                &source_display.area,
                source_display.area_max,
                &candidate_display.area,
                candidate_display.area_max,
                direction,
            ) {
                let distance = area_distance_in_direction(
                    &source_display.area,
                    source_display.area_max,
                    &candidate_display.area,
                    candidate_display.area_max,
                    direction,
                );
                if distance < best_distance {
                    best_index = index;
                    best_distance = distance;
                }
            }
        }

        best_index
    }

    #[test]
    fn test_closest_display_in_direction() {
        let display_list = init_test_display_list();
        let display_count = display_list.len() as i32;
        let mut best_index;

        best_index = closest_display_in_direction(&display_list, display_count, 0, DIR_WEST);
        assert_eq!(best_index, 1);

        best_index = closest_display_in_direction(&display_list, display_count, 1, DIR_WEST);
        assert_eq!(best_index, -1);

        best_index = closest_display_in_direction(&display_list, display_count, 2, DIR_WEST);
        assert_eq!(best_index, 0);

        best_index = closest_display_in_direction(&display_list, display_count, 0, DIR_EAST);
        assert_eq!(best_index, 2);

        best_index = closest_display_in_direction(&display_list, display_count, 1, DIR_EAST);
        assert_eq!(best_index, 0);

        best_index = closest_display_in_direction(&display_list, display_count, 2, DIR_EAST);
        assert_eq!(best_index, -1);
    }

    fn area_at(x: f32, y: f32, width: f32, height: f32) -> Area {
        Area {
            x,
            y,
            width,
            height,
        }
    }

    fn x_y_width_height(area: Area) -> (f32, f32, f32, f32) {
        (area.x, area.y, area.width, area.height)
    }

    fn assert_pair_is(
        split: WindowNodeSplit,
        gap: i32,
        ratio: f32,
        parent_area: Area,
        expected_first_area: (f32, f32, f32, f32),
        expected_second_area: (f32, f32, f32, f32),
    ) {
        let (first_area, second_area) = area_make_pair(split, gap, ratio, parent_area);

        assert_eq!(
            (x_y_width_height(first_area), x_y_width_height(second_area)),
            (expected_first_area, expected_second_area),
            "gap {gap} ratio {ratio} parent {:?}",
            x_y_width_height(parent_area)
        );
    }

    #[test]
    fn area_make_pair_for_a_y_split_truncates_both_widths_and_rounds_the_offset_of_the_second_area()
    {
        assert_pair_is(
            WindowNodeSplit::Y,
            10,
            0.5,
            area_at(0.0, 0.0, 1001.0, 500.0),
            (0.0, 0.0, 495.0, 500.0),
            (506.0, 0.0, 495.0, 500.0),
        );
        assert_pair_is(
            WindowNodeSplit::Y,
            10,
            0.6,
            area_at(0.0, 25.0, 1001.0, 500.0),
            (0.0, 25.0, 594.0, 500.0),
            (605.0, 25.0, 396.0, 500.0),
        );
        assert_pair_is(
            WindowNodeSplit::Y,
            7,
            0.3333,
            area_at(-1728.0, 38.0, 1728.0, 1079.0),
            (-1728.0, 38.0, 573.0, 1079.0),
            (-1147.0, 38.0, 1147.0, 1079.0),
        );
    }

    #[test]
    fn area_make_pair_for_a_y_split_keeps_a_fractional_origin_of_the_parent() {
        assert_pair_is(
            WindowNodeSplit::Y,
            0,
            0.5,
            area_at(100.25, 30.0, 1439.0, 900.0),
            (100.25, 30.0, 719.0, 900.0),
            (820.25, 30.0, 719.0, 900.0),
        );
    }

    #[test]
    fn area_make_pair_for_an_x_split_truncates_both_heights_and_rounds_the_offset_of_the_second_area()
     {
        assert_pair_is(
            WindowNodeSplit::X,
            10,
            0.5,
            area_at(0.0, 0.0, 800.0, 1001.0),
            (0.0, 0.0, 800.0, 495.0),
            (0.0, 506.0, 800.0, 495.0),
        );
        assert_pair_is(
            WindowNodeSplit::X,
            10,
            0.6,
            area_at(50.0, 25.0, 800.0, 1001.0),
            (50.0, 25.0, 800.0, 594.0),
            (50.0, 630.0, 800.0, 396.0),
        );
        assert_pair_is(
            WindowNodeSplit::X,
            12,
            0.1,
            area_at(0.0, 38.5, 1512.0, 944.0),
            (0.0, 38.5, 1512.0, 93.0),
            (0.0, 143.5, 1512.0, 838.0),
        );
        assert_pair_is(
            WindowNodeSplit::X,
            0,
            0.9,
            area_at(0.0, 0.0, 1512.0, 945.0),
            (0.0, 0.0, 1512.0, 850.0),
            (0.0, 851.0, 1512.0, 94.0),
        );
    }

    #[test]
    fn area_make_pair_splits_along_x_for_every_split_other_than_y() {
        for split in [
            WindowNodeSplit::X,
            WindowNodeSplit::Auto,
            WindowNodeSplit::None,
        ] {
            assert_pair_is(
                split,
                4,
                0.5,
                area_at(0.0, 0.0, 300.0, 301.0),
                (0.0, 0.0, 300.0, 148.0),
                (0.0, 153.0, 300.0, 148.0),
            );
        }
    }

    #[test]
    fn area_max_point_is_the_last_pixel_inside_the_area_computed_in_f32() {
        let expected_max_points = [
            (area_at(10.0, 20.0, 100.0, 50.0), (109.0, 69.0)),
            (area_at(0.5, -10.25, 3.0, 1.0), (2.5, -10.25)),
            (area_at(16777216.0, 0.0, 1.0, 1.0), (16777215.0, 0.0)),
            (area_at(-1728.0, 0.0, 1728.0, 1117.0), (-1.0, 1116.0)),
            (area_at(0.0, 0.0, 0.0, 0.0), (-1.0, -1.0)),
        ];

        for (area, (expected_x, expected_y)) in expected_max_points {
            let max_point = area_max_point(area);
            assert_eq!(
                (max_point.x, max_point.y),
                (expected_x, expected_y),
                "area {:?}",
                x_y_width_height(area)
            );
        }
    }
}
