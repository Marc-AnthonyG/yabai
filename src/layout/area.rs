use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize};
use crate::layout::tree::{
    WindowNodeChild, WindowNodeSplit,
    window_node_split_and_child_placing_a_window_inserted_in_direction,
};
use crate::support::direction::{
    DIRECTION_EAST, DIRECTION_NORTH, DIRECTION_SOUTH, DIRECTION_STACK_INSTEAD_OF_SPLIT,
    DIRECTION_WEST,
};

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

pub(crate) fn cgrect_from_area(area: Area) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: area.x as f64,
            y: area.y as f64,
        },
        size: CGSize {
            width: area.width as f64,
            height: area.height as f64,
        },
    }
}

pub(crate) fn bottom_right_pixel_inside_area(area: Area) -> CGPoint {
    CGPoint {
        x: (area.x + area.width - 1.0f32) as f64,
        y: (area.y + area.height - 1.0f32) as f64,
    }
}

pub(crate) fn divide_area_into_two_by_split_ratio_and_gap(
    split: WindowNodeSplit,
    gap: i32,
    ratio: f32,
    parent_area: Area,
) -> (Area, Area) {
    if split == WindowNodeSplit::Vertical {
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

pub(crate) fn area_a_window_inserted_in_direction_takes_from_node_area(
    insert_direction: i32,
    node_area: Area,
    ratio: f32,
    gap: i32,
) -> Option<Area> {
    if insert_direction == DIRECTION_STACK_INSTEAD_OF_SPLIT {
        return Some(node_area);
    }
    let (split, child_of_the_inserted_window) =
        window_node_split_and_child_placing_a_window_inserted_in_direction(insert_direction)?;

    let (first_child_area, second_child_area) =
        divide_area_into_two_by_split_ratio_and_gap(split, gap, ratio, node_area);
    if child_of_the_inserted_window == WindowNodeChild::Second {
        Some(second_child_area)
    } else {
        Some(first_child_area)
    }
}

pub(crate) fn is_target_area_in_direction_of_source_area_and_facing_it(
    first_area: &Area,
    first_area_max_point: CGPoint,
    second_area: &Area,
    second_area_max_point: CGPoint,
    direction: i32,
) -> bool {
    if direction == DIRECTION_NORTH && first_area_max_point.y <= second_area.y as f64 {
        return false;
    }
    if direction == DIRECTION_EAST && second_area_max_point.x <= first_area.x as f64 {
        return false;
    }
    if direction == DIRECTION_SOUTH && second_area_max_point.y <= first_area.y as f64 {
        return false;
    }
    if direction == DIRECTION_WEST && first_area_max_point.x <= second_area.x as f64 {
        return false;
    }

    if direction == DIRECTION_NORTH || direction == DIRECTION_SOUTH {
        return (second_area_max_point.x > first_area.x as f64
            && second_area_max_point.x <= first_area_max_point.x)
            || (second_area.x < first_area.x
                && second_area_max_point.x > first_area_max_point.x)
            || (second_area.x >= first_area.x
                && (second_area.x as f64) < first_area_max_point.x);
    }

    if direction == DIRECTION_EAST || direction == DIRECTION_WEST {
        return (second_area_max_point.y > first_area.y as f64
            && second_area_max_point.y <= first_area_max_point.y)
            || (second_area.y < first_area.y
                && second_area_max_point.y > first_area_max_point.y)
            || (second_area.y >= first_area.y
                && (second_area.y as f64) < first_area_max_point.y);
    }

    false
}

pub(crate) fn distance_from_source_area_to_target_area_in_direction(
    first_area: &Area,
    first_area_max_point: CGPoint,
    second_area: &Area,
    second_area_max_point: CGPoint,
    direction: i32,
) -> i32 {
    match direction {
        DIRECTION_NORTH => {
            return (if second_area_max_point.y > first_area.y as f64 {
                second_area_max_point.y - first_area.y as f64
            } else {
                first_area.y as f64 - second_area_max_point.y
            }) as i32;
        }
        DIRECTION_EAST => {
            return (if (second_area.x as f64) < first_area_max_point.x {
                first_area_max_point.x - second_area.x as f64
            } else {
                second_area.x as f64 - first_area_max_point.x
            }) as i32;
        }
        DIRECTION_SOUTH => {
            return (if (second_area.y as f64) < first_area_max_point.y {
                first_area_max_point.y - second_area.y as f64
            } else {
                second_area.y as f64 - first_area_max_point.y
            }) as i32;
        }
        DIRECTION_WEST => {
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

pub(crate) fn is_difference_beyond_accessibility_rounding(first: f64, second: f64) -> bool {
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
        Area, area_a_window_inserted_in_direction_takes_from_node_area, area_from_cgrect,
        bottom_right_pixel_inside_area, cgrect_from_area,
        distance_from_source_area_to_target_area_in_direction,
        divide_area_into_two_by_split_ratio_and_gap,
        is_target_area_in_direction_of_source_area_and_facing_it,
    };
    use crate::ffi::core_foundation::{CGPoint, CGRect, CGSize};
    use crate::layout::tree::WindowNodeSplit;
    use crate::support::direction::{
        DIRECTION_EAST, DIRECTION_NORTH, DIRECTION_SOUTH, DIRECTION_STACK_INSTEAD_OF_SPLIT,
        DIRECTION_WEST,
    };

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
        display_list[0].area_max = bottom_right_pixel_inside_area(display_list[0].area);

        display_list[1].area.x = -1728.0;
        display_list[1].area.y = 0.0;
        display_list[1].area.width = 1728.0;
        display_list[1].area.height = 1117.0;
        display_list[1].area_max = bottom_right_pixel_inside_area(display_list[1].area);

        display_list[2].area.x = 2560.0;
        display_list[2].area.y = 0.0;
        display_list[2].area.width = 1920.0;
        display_list[2].area.height = 1080.0;
        display_list[2].area_max = bottom_right_pixel_inside_area(display_list[2].area);

        display_list
    }

    #[test]
    fn test_display_is_target_area_in_direction_of_source_area_and_facing_it() {
        let display_list = init_test_display_list();

        let t1 = is_target_area_in_direction_of_source_area_and_facing_it(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[1].area,
            display_list[1].area_max,
            DIRECTION_WEST,
        );
        assert_eq!(t1, true);

        let t2 = is_target_area_in_direction_of_source_area_and_facing_it(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[1].area,
            display_list[1].area_max,
            DIRECTION_EAST,
        );
        assert_eq!(t2, false);

        let t3 = is_target_area_in_direction_of_source_area_and_facing_it(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[2].area,
            display_list[2].area_max,
            DIRECTION_WEST,
        );
        assert_eq!(t3, false);

        let t4 = is_target_area_in_direction_of_source_area_and_facing_it(
            &display_list[0].area,
            display_list[0].area_max,
            &display_list[2].area,
            display_list[2].area_max,
            DIRECTION_EAST,
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
            if is_target_area_in_direction_of_source_area_and_facing_it(
                &source_display.area,
                source_display.area_max,
                &candidate_display.area,
                candidate_display.area_max,
                direction,
            ) {
                let distance = distance_from_source_area_to_target_area_in_direction(
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

        best_index = closest_display_in_direction(&display_list, display_count, 0, DIRECTION_WEST);
        assert_eq!(best_index, 1);

        best_index = closest_display_in_direction(&display_list, display_count, 1, DIRECTION_WEST);
        assert_eq!(best_index, -1);

        best_index = closest_display_in_direction(&display_list, display_count, 2, DIRECTION_WEST);
        assert_eq!(best_index, 0);

        best_index = closest_display_in_direction(&display_list, display_count, 0, DIRECTION_EAST);
        assert_eq!(best_index, 2);

        best_index = closest_display_in_direction(&display_list, display_count, 1, DIRECTION_EAST);
        assert_eq!(best_index, 0);

        best_index = closest_display_in_direction(&display_list, display_count, 2, DIRECTION_EAST);
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
        let (first_area, second_area) =
            divide_area_into_two_by_split_ratio_and_gap(split, gap, ratio, parent_area);

        assert_eq!(
            (x_y_width_height(first_area), x_y_width_height(second_area)),
            (expected_first_area, expected_second_area),
            "gap {gap} ratio {ratio} parent {:?}",
            x_y_width_height(parent_area)
        );
    }

    #[test]
    fn divide_area_into_two_by_split_ratio_and_gap_for_a_y_split_truncates_both_widths_and_rounds_the_offset_of_the_second_area()
    {
        assert_pair_is(
            WindowNodeSplit::Vertical,
            10,
            0.5,
            area_at(0.0, 0.0, 1001.0, 500.0),
            (0.0, 0.0, 495.0, 500.0),
            (506.0, 0.0, 495.0, 500.0),
        );
        assert_pair_is(
            WindowNodeSplit::Vertical,
            10,
            0.6,
            area_at(0.0, 25.0, 1001.0, 500.0),
            (0.0, 25.0, 594.0, 500.0),
            (605.0, 25.0, 396.0, 500.0),
        );
        assert_pair_is(
            WindowNodeSplit::Vertical,
            7,
            0.3333,
            area_at(-1728.0, 38.0, 1728.0, 1079.0),
            (-1728.0, 38.0, 573.0, 1079.0),
            (-1147.0, 38.0, 1147.0, 1079.0),
        );
    }

    #[test]
    fn divide_area_into_two_by_split_ratio_and_gap_for_a_y_split_keeps_a_fractional_origin_of_the_parent()
     {
        assert_pair_is(
            WindowNodeSplit::Vertical,
            0,
            0.5,
            area_at(100.25, 30.0, 1439.0, 900.0),
            (100.25, 30.0, 719.0, 900.0),
            (820.25, 30.0, 719.0, 900.0),
        );
    }

    #[test]
    fn divide_area_into_two_by_split_ratio_and_gap_for_an_x_split_truncates_both_heights_and_rounds_the_offset_of_the_second_area()
     {
        assert_pair_is(
            WindowNodeSplit::Horizontal,
            10,
            0.5,
            area_at(0.0, 0.0, 800.0, 1001.0),
            (0.0, 0.0, 800.0, 495.0),
            (0.0, 506.0, 800.0, 495.0),
        );
        assert_pair_is(
            WindowNodeSplit::Horizontal,
            10,
            0.6,
            area_at(50.0, 25.0, 800.0, 1001.0),
            (50.0, 25.0, 800.0, 594.0),
            (50.0, 630.0, 800.0, 396.0),
        );
        assert_pair_is(
            WindowNodeSplit::Horizontal,
            12,
            0.1,
            area_at(0.0, 38.5, 1512.0, 944.0),
            (0.0, 38.5, 1512.0, 93.0),
            (0.0, 143.5, 1512.0, 838.0),
        );
        assert_pair_is(
            WindowNodeSplit::Horizontal,
            0,
            0.9,
            area_at(0.0, 0.0, 1512.0, 945.0),
            (0.0, 0.0, 1512.0, 850.0),
            (0.0, 851.0, 1512.0, 94.0),
        );
    }

    #[test]
    fn divide_area_into_two_by_split_ratio_and_gap_splits_along_x_for_every_split_other_than_y() {
        for split in [
            WindowNodeSplit::Horizontal,
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
    fn bottom_right_pixel_inside_area_is_the_last_pixel_inside_the_area_computed_in_f32() {
        let expected_max_points = [
            (area_at(10.0, 20.0, 100.0, 50.0), (109.0, 69.0)),
            (area_at(0.5, -10.25, 3.0, 1.0), (2.5, -10.25)),
            (area_at(16777216.0, 0.0, 1.0, 1.0), (16777215.0, 0.0)),
            (area_at(-1728.0, 0.0, 1728.0, 1117.0), (-1.0, 1116.0)),
            (area_at(0.0, 0.0, 0.0, 0.0), (-1.0, -1.0)),
        ];

        for (area, (expected_x, expected_y)) in expected_max_points {
            let max_point = bottom_right_pixel_inside_area(area);
            assert_eq!(
                (max_point.x, max_point.y),
                (expected_x, expected_y),
                "area {:?}",
                x_y_width_height(area)
            );
        }
    }

    fn area_a_window_inserted_takes(
        insert_direction: i32,
        node_area: Area,
        ratio: f32,
        gap: i32,
    ) -> Option<(f32, f32, f32, f32)> {
        area_a_window_inserted_in_direction_takes_from_node_area(
            insert_direction,
            node_area,
            ratio,
            gap,
        )
        .map(x_y_width_height)
    }

    fn assert_each_direction_inserts_into(
        node_area: Area,
        ratio: f32,
        gap: i32,
        expected_area_for_north_east_south_and_west: [(f32, f32, f32, f32); 4],
    ) {
        for (insert_direction, expected_area) in [
            DIRECTION_NORTH,
            DIRECTION_EAST,
            DIRECTION_SOUTH,
            DIRECTION_WEST,
        ]
        .into_iter()
        .zip(expected_area_for_north_east_south_and_west)
        {
            assert_eq!(
                area_a_window_inserted_takes(insert_direction, node_area, ratio, gap),
                Some(expected_area),
                "direction {insert_direction} node {:?} ratio {ratio} gap {gap}",
                x_y_width_height(node_area)
            );
        }
    }

    #[test]
    fn a_window_inserted_north_or_south_takes_the_top_or_bottom_of_the_node_and_west_or_east_its_left_or_right()
     {
        assert_each_direction_inserts_into(
            area_at(100.0, 60.0, 1001.0, 701.0),
            0.5,
            10,
            [
                (100.0, 60.0, 1001.0, 345.0),
                (606.0, 60.0, 495.0, 701.0),
                (100.0, 416.0, 1001.0, 345.0),
                (100.0, 60.0, 495.0, 701.0),
            ],
        );
    }

    #[test]
    fn an_odd_node_with_a_gap_is_divided_with_the_truncation_and_rounding_of_the_c_split() {
        assert_each_direction_inserts_into(
            area_at(-1728.0, 38.0, 1727.0, 1079.0),
            0.37,
            7,
            [
                (-1728.0, 38.0, 1727.0, 396.0),
                (-1085.0, 38.0, 1083.0, 1079.0),
                (-1728.0, 442.0, 1727.0, 675.0),
                (-1728.0, 38.0, 636.0, 1079.0),
            ],
        );
        assert_each_direction_inserts_into(
            area_at(0.5, 25.25, 333.0, 211.0),
            0.62,
            13,
            [
                (0.5, 25.25, 333.0, 122.0),
                (211.5, 25.25, 121.0, 211.0),
                (0.5, 161.25, 333.0, 75.0),
                (0.5, 25.25, 198.0, 211.0),
            ],
        );
        assert_each_direction_inserts_into(
            area_at(0.0, 0.0, 3.0, 5.0),
            0.5,
            0,
            [
                (0.0, 0.0, 3.0, 2.0),
                (2.0, 0.0, 1.0, 5.0),
                (0.0, 3.0, 3.0, 2.0),
                (0.0, 0.0, 1.0, 5.0),
            ],
        );
    }

    #[test]
    fn a_window_inserted_as_a_stack_takes_the_whole_node() {
        let node_area = area_at(-1728.0, 38.5, 1727.0, 1079.0);

        assert_eq!(
            area_a_window_inserted_takes(DIRECTION_STACK_INSTEAD_OF_SPLIT, node_area, 0.37, 7),
            Some(x_y_width_height(node_area))
        );
    }

    #[test]
    fn an_insert_direction_that_is_neither_a_compass_direction_nor_a_stack_predicts_no_area() {
        for insert_direction in [0, 1, 45, 91, 179, 269, 359, 361, -90, i32::MIN, i32::MAX] {
            assert_eq!(
                area_a_window_inserted_takes(
                    insert_direction,
                    area_at(0.0, 0.0, 1000.0, 800.0),
                    0.5,
                    10
                ),
                None,
                "direction {insert_direction}"
            );
        }
    }

    #[test]
    fn an_area_survives_a_round_trip_through_a_cgrect_bit_for_bit() {
        for area in [
            area_at(0.0, 0.0, 0.0, 0.0),
            area_at(-1728.0, 38.5, 1727.0, 1079.0),
            area_at(0.1, -0.3, 16777217.0, 1e-7),
            area_at(f32::MIN_POSITIVE, -f32::MAX, f32::MAX, 3.4028235e38),
        ] {
            let round_tripped_area = area_from_cgrect(cgrect_from_area(area));

            assert_eq!(
                [
                    round_tripped_area.x.to_bits(),
                    round_tripped_area.y.to_bits(),
                    round_tripped_area.width.to_bits(),
                    round_tripped_area.height.to_bits(),
                ],
                [
                    area.x.to_bits(),
                    area.y.to_bits(),
                    area.width.to_bits(),
                    area.height.to_bits(),
                ],
                "area {:?}",
                x_y_width_height(area)
            );
        }
    }

    #[test]
    fn a_cgrect_of_values_an_f32_holds_survives_a_round_trip_through_an_area() {
        let rect = CGRect {
            origin: CGPoint {
                x: -1728.0,
                y: 38.5,
            },
            size: CGSize {
                width: 1727.25,
                height: 0.1f32 as f64,
            },
        };

        let round_tripped_rect = cgrect_from_area(area_from_cgrect(rect));

        assert_eq!(
            [
                round_tripped_rect.origin.x,
                round_tripped_rect.origin.y,
                round_tripped_rect.size.width,
                round_tripped_rect.size.height,
            ],
            [
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                rect.size.height
            ]
        );
    }

    #[test]
    fn a_cgrect_value_an_f32_cannot_hold_is_rounded_to_the_nearest_f32() {
        let rect = CGRect {
            origin: CGPoint {
                x: 0.1,
                y: 16777217.0,
            },
            size: CGSize {
                width: 1.0 / 3.0,
                height: 2.5,
            },
        };

        let area = area_from_cgrect(rect);

        assert_eq!(
            x_y_width_height(area),
            (0.1f32, 16777216.0f32, 1.0f32 / 3.0f32, 2.5f32)
        );
    }
}
