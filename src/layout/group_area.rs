use crate::layout::area::Area;
use crate::layout::group::is_node_a_group;
use crate::layout::tree::WindowNode;
use crate::layout::view::View;
use crate::window::manager::WindowManager;

pub(crate) fn area_of_tile_including_its_zoom(view: &View, node: &WindowNode) -> Area {
    match node.zoom.and_then(|zoom| view.find_node(zoom)) {
        Some(zoom_node) => zoom_node.area,
        None => node.area,
    }
}

pub(crate) fn header_height_that_fits_in_tile(tile_area: Area, header_height: f32) -> f32 {
    header_height.min(0.5 * tile_area.height).max(0.0)
}

pub(crate) fn area_left_for_windows_below_a_header(tile_area: Area, header_height: f32) -> Area {
    let header_height = header_height_that_fits_in_tile(tile_area, header_height);
    Area {
        x: tile_area.x,
        y: tile_area.y + header_height,
        width: tile_area.width,
        height: tile_area.height - header_height,
    }
}

pub(crate) fn area_of_the_header_of_a_tile(tile_area: Area, header_height: f32) -> Area {
    Area {
        x: tile_area.x,
        y: tile_area.y,
        width: tile_area.width,
        height: header_height_that_fits_in_tile(tile_area, header_height),
    }
}

pub(crate) fn area_given_to_the_windows_of_tile(
    tile_area: Area,
    view: &View,
    node: &WindowNode,
    window_manager: &WindowManager,
) -> Area {
    if is_node_a_group(view, node, window_manager) {
        area_left_for_windows_below_a_header(tile_area, window_manager.group_header_style.height)
    } else {
        tile_area
    }
}

pub(crate) fn area_given_to_the_windows_of_node(
    view: &View,
    node: &WindowNode,
    window_manager: &WindowManager,
) -> Area {
    area_given_to_the_windows_of_tile(
        area_of_tile_including_its_zoom(view, node),
        view,
        node,
        window_manager,
    )
}

#[cfg(test)]
mod tests {
    use super::{area_left_for_windows_below_a_header, area_of_the_header_of_a_tile};
    use crate::layout::area::Area;

    fn tile_of_800_by_600_at_100_40() -> Area {
        Area {
            x: 100.0,
            y: 40.0,
            width: 800.0,
            height: 600.0,
        }
    }

    fn corners(area: Area) -> (f32, f32, f32, f32) {
        (area.x, area.y, area.width, area.height)
    }

    #[test]
    fn a_group_header_takes_the_top_of_the_tile_and_the_windows_get_the_rest() {
        let tile = tile_of_800_by_600_at_100_40();

        assert_eq!(
            corners(area_of_the_header_of_a_tile(tile, 24.0)),
            (100.0, 40.0, 800.0, 24.0)
        );
        assert_eq!(
            corners(area_left_for_windows_below_a_header(tile, 24.0)),
            (100.0, 64.0, 800.0, 576.0)
        );
    }

    #[test]
    fn a_header_never_takes_more_than_half_of_a_short_tile() {
        let short_tile = Area {
            height: 30.0,
            ..tile_of_800_by_600_at_100_40()
        };

        assert_eq!(area_of_the_header_of_a_tile(short_tile, 24.0).height, 15.0);
        assert_eq!(
            corners(area_left_for_windows_below_a_header(short_tile, 24.0)),
            (100.0, 55.0, 800.0, 15.0)
        );
    }

    #[test]
    fn a_header_height_of_zero_leaves_the_whole_tile_to_the_windows() {
        let tile = tile_of_800_by_600_at_100_40();

        assert_eq!(
            corners(area_left_for_windows_below_a_header(tile, 0.0)),
            corners(tile)
        );
    }
}
