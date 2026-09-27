use core::ffi::{c_int, c_void};
use core::ptr::NonNull;

use crate::display::{
    display_center, display_handler, display_id, display_serialize, display_space_id, display_uuid,
};
use crate::ffi::CFStringOwned;
use crate::ffi::accessibility::{
    AXUIElement, AXUIElementCopyAttributeValue, AXUIElementCopyElementAtPosition, ax_window_id,
    kAXRoleAttribute, kAXWindowAttribute, kAXWindowRole,
};
use crate::ffi::carbon_process::{
    CoreDockGetAutoHideEnabled, CoreDockGetOrientationAndPinning, ProcessSerialNumber,
};
use crate::ffi::core_foundation::{
    CFArray, CFArrayCreateMutableCopy, CFArrayGetCount, CFArraySortValues, CFComparisonResult,
    CFEqual, CFIndex, CFRangeMake, CFRetained, CFString, CFType, CGFloat, CGPoint,
    CGRect, SendCFRetained, as_cftype, cfarray_borrow_value_at_index, take_create_rule_result,
};
use crate::ffi::core_graphics::{
    CGDisplayBounds, CGDisplayRegisterReconfigurationCallback, CGGetActiveDisplayList,
    CGMainDisplayID, CGPostMouseEvent, CGWarpMouseCursorPosition, kCGErrorSuccess,
};
use crate::ffi::skylight::{
    SLSCopyActiveMenuBarDisplayIdentifier, SLSCopyBestManagedDisplayForPoint,
    SLSCopyBestManagedDisplayForRect, SLSCopyManagedDisplays, SLSGetConnectionPSN,
    SLSGetCurrentCursorLocation, SLSGetDockRectWithReason, SLSGetMenuBarAutohideEnabled,
    SLSGetWindowOwner, SLSManagedDisplayIsAnimating, SLSSetActiveMenuBarDisplayIdentifier,
};
use crate::globals::CONNECTION;
use crate::handles::{DisplayId, SpaceId, WindowId};
use crate::misc::helpers::string_equals;
use crate::misc::macros::in_range_ie;
use crate::misc::response::Response;
use crate::mission_control::{MissionControlMode, mission_control_is_active};
use crate::sa::scripting_addition_focus_space;
use crate::space::space_display_id;
use crate::space_manager::{SpaceOpError, space_manager_active_space};
use crate::view::{
    area_distance_in_direction, area_from_cgrect, area_is_in_direction, area_max_point,
};
use crate::window_manager::{
    WindowManager, window_manager_center_mouse,
    window_manager_find_window_on_space_by_rank_filtering_window,
    window_manager_focus_window_with_raise,
};
use crate::workspace::{
    workspace_is_macos_bigsur, workspace_is_macos_monterey, workspace_is_macos_ventura,
};

#[cfg(target_arch = "aarch64")]
use crate::ffi::skylight::SLSGetDisplayMenubarHeight;
#[cfg(target_arch = "x86_64")]
use crate::ffi::skylight::SLSGetRevealedMenuBarBounds;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(usize)]
pub(crate) enum DisplayArrangementOrder {
    #[default]
    Default = 0,
    X = 1,
    Y = 2,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum ExternalBarMode {
    #[default]
    Off = 0,
    Main = 1,
    All = 2,
}

pub(crate) struct DisplayLabel {
    pub(crate) display_id: DisplayId,
    pub(crate) label: String,
}

#[derive(Default)]
pub(crate) struct DisplayManager {
    pub(crate) current_display_id: DisplayId,
    pub(crate) last_display_id: DisplayId,
    pub(crate) top_padding: i32,
    pub(crate) bottom_padding: i32,
    pub(crate) order: DisplayArrangementOrder,
    pub(crate) mode: ExternalBarMode,
    pub(crate) labels: Vec<DisplayLabel>,
}

pub(crate) const DOCK_ORIENTATION_BOTTOM: i32 = 2;
pub(crate) const DOCK_ORIENTATION_LEFT: i32 = 3;
pub(crate) const DOCK_ORIENTATION_RIGHT: i32 = 4;

pub(crate) static DISPLAY_ARRANGEMENT_ORDER_STR: [&str; 3] = ["default", "horizontal", "vertical"];

pub(crate) static EXTERNAL_BAR_MODE_STR: [&str; 3] = ["off", "main", "all"];

pub(crate) fn display_manager_query_displays(
    response: &mut Response,
    flags: u64,
    display_manager: &mut DisplayManager,
) -> bool {
    let display_list = display_manager_active_display_list();
    let count = display_list.len() as i32;

    response.write(format_args!("["));
    for index in 0..count {
        display_serialize(
            response,
            display_list[index as usize],
            flags,
            display_manager,
        );
        response.write(format_args!(
            "{}",
            if index < count - 1 { ',' } else { ']' }
        ));
    }
    response.write(format_args!("\n"));

    true
}

pub(crate) fn display_manager_get_label_for_display(
    display_manager: &mut DisplayManager,
    display_id: DisplayId,
) -> Option<&mut DisplayLabel> {
    for display_label in display_manager.labels.iter_mut() {
        if display_label.display_id == display_id {
            return Some(display_label);
        }
    }

    None
}

pub(crate) fn display_manager_get_display_for_label<'display_manager>(
    display_manager: &'display_manager mut DisplayManager,
    label: &[u8],
) -> Option<&'display_manager mut DisplayLabel> {
    let label = String::from_utf8_lossy(label);

    for display_label in display_manager.labels.iter_mut() {
        if string_equals(Some(&label), Some(&display_label.label)) {
            return Some(display_label);
        }
    }

    None
}

pub(crate) fn display_manager_remove_label_for_display(
    display_manager: &mut DisplayManager,
    display_id: DisplayId,
) -> bool {
    for index in 0..display_manager.labels.len() {
        let display_label = &display_manager.labels[index];
        if display_label.display_id == display_id {
            display_manager.labels.swap_remove(index);
            return true;
        }
    }

    false
}

pub(crate) fn display_manager_set_label_for_display(
    display_manager: &mut DisplayManager,
    display_id: DisplayId,
    label: String,
) {
    display_manager_remove_label_for_display(display_manager, display_id);

    for index in 0..display_manager.labels.len() {
        let display_label = &display_manager.labels[index];
        if display_label.label == label {
            display_manager.labels.swap_remove(index);
            break;
        }
    }

    display_manager.labels.push(DisplayLabel { display_id, label });
}

pub(crate) fn display_manager_main_display_id() -> DisplayId {
    DisplayId(CGMainDisplayID())
}

pub(crate) fn display_manager_active_display_uuid() -> Option<CFStringOwned> {
    let connection_id = *CONNECTION.get().unwrap();
    unsafe { take_create_rule_result(SLSCopyActiveMenuBarDisplayIdentifier(connection_id)) }
        .map(SendCFRetained)
}

pub(crate) fn display_manager_active_display_id() -> DisplayId {
    let uuid = display_manager_active_display_uuid();
    debug_assert!(uuid.is_some());

    let Some(uuid) = uuid else {
        return DisplayId(0);
    };

    display_id(uuid.as_ref())
}

pub(crate) fn display_manager_dock_display_uuid() -> Option<CFStringOwned> {
    let connection_id = *CONNECTION.get().unwrap();
    let dock = display_manager_dock_rect();
    unsafe { take_create_rule_result(SLSCopyBestManagedDisplayForRect(connection_id, dock)) }
        .map(SendCFRetained)
}

pub(crate) fn display_manager_dock_display_id() -> DisplayId {
    let Some(uuid) = display_manager_dock_display_uuid() else {
        return DisplayId(0);
    };

    display_id(uuid.as_ref())
}

pub(crate) fn display_manager_point_display_uuid(point: CGPoint) -> Option<CFStringOwned> {
    let connection_id = *CONNECTION.get().unwrap();
    unsafe { take_create_rule_result(SLSCopyBestManagedDisplayForPoint(connection_id, point)) }
        .map(SendCFRetained)
}

pub(crate) fn display_manager_point_display_id(point: CGPoint) -> DisplayId {
    let Some(uuid) = display_manager_point_display_uuid(point) else {
        return DisplayId(0);
    };

    display_id(uuid.as_ref())
}

pub(crate) unsafe extern "C-unwind" fn display_manager_coordinate_comparator(
    a_display: *const c_void,
    b_display: *const c_void,
    context: *mut c_void,
) -> CFComparisonResult {
    let axis = context as usize as u32;

    let a_display_id = display_id(unsafe { &*a_display.cast::<CFString>() });
    let b_display_id = display_id(unsafe { &*b_display.cast::<CFString>() });

    let a_center = display_center(a_display_id);
    let b_center = display_center(b_display_id);

    let mut a_coordinate: f32 = if axis == DisplayArrangementOrder::Y as u32 {
        a_center.y as f32
    } else {
        a_center.x as f32
    };
    let mut b_coordinate: f32 = if axis == DisplayArrangementOrder::Y as u32 {
        b_center.y as f32
    } else {
        b_center.x as f32
    };

    if a_coordinate < b_coordinate {
        return CFComparisonResult::CompareLessThan;
    }
    if a_coordinate > b_coordinate {
        return CFComparisonResult::CompareGreaterThan;
    }

    a_coordinate = if axis == DisplayArrangementOrder::Y as u32 {
        a_center.x as f32
    } else {
        a_center.y as f32
    };
    b_coordinate = if axis == DisplayArrangementOrder::Y as u32 {
        b_center.x as f32
    } else {
        b_center.y as f32
    };

    if a_coordinate < b_coordinate {
        return CFComparisonResult::CompareLessThan;
    }
    if a_coordinate > b_coordinate {
        return CFComparisonResult::CompareGreaterThan;
    }

    CFComparisonResult::CompareEqualTo
}

pub(crate) fn display_manager_display_id_arrangement(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
) -> i32 {
    let connection_id = *CONNECTION.get().unwrap();
    let mut result: i32 = 0;

    let Some(uuid) = display_uuid(display_id) else {
        return result;
    };

    let Some(mut displays) =
        (unsafe { take_create_rule_result(SLSCopyManagedDisplays(connection_id)) })
    else {
        return result;
    };

    let count = CFArrayGetCount(&displays) as i32;
    if count == 0 {
        return result;
    }

    if display_manager.order != DisplayArrangementOrder::Default {
        let mutable_displays =
            unsafe { CFArrayCreateMutableCopy(None, count as CFIndex, Some(&displays)) };
        if let Some(mutable_displays) = mutable_displays {
            unsafe {
                CFArraySortValues(
                    Some(&mutable_displays),
                    CFRangeMake(0, count as CFIndex),
                    Some(display_manager_coordinate_comparator),
                    display_manager.order as usize as *mut c_void,
                )
            };
            drop(displays);
            displays = unsafe { CFRetained::cast_unchecked::<CFArray>(mutable_displays) };
        }
    }

    for index in 0..count {
        if CFEqual(
            unsafe { cfarray_borrow_value_at_index::<CFType>(&displays, index as CFIndex) },
            Some(as_cftype(uuid.as_ref())),
        ) {
            result = index + 1;
            break;
        }
    }

    result
}

pub(crate) fn display_manager_arrangement_display_uuid(
    arrangement: i32,
    display_manager: &mut DisplayManager,
) -> Option<CFStringOwned> {
    let connection_id = *CONNECTION.get().unwrap();
    let mut result: Option<CFStringOwned> = None;

    let Some(mut displays) =
        (unsafe { take_create_rule_result(SLSCopyManagedDisplays(connection_id)) })
    else {
        return result;
    };

    let count = CFArrayGetCount(&displays) as i32;
    let index = arrangement - 1;

    if in_range_ie(index, 0, count) {
        if display_manager.order != DisplayArrangementOrder::Default {
            let mutable_displays =
                unsafe { CFArrayCreateMutableCopy(None, count as CFIndex, Some(&displays)) };
            if let Some(mutable_displays) = mutable_displays {
                unsafe {
                    CFArraySortValues(
                        Some(&mutable_displays),
                        CFRangeMake(0, count as CFIndex),
                        Some(display_manager_coordinate_comparator),
                        display_manager.order as usize as *mut c_void,
                    )
                };
                drop(displays);
                displays = unsafe { CFRetained::cast_unchecked::<CFArray>(mutable_displays) };
            }
        }

        result = unsafe { cfarray_borrow_value_at_index::<CFString>(&displays, index as CFIndex) }
            .map(|value| SendCFRetained(unsafe { CFRetained::retain(NonNull::from(value)) }));
    }

    result
}

pub(crate) fn display_manager_arrangement_display_id(
    arrangement: i32,
    display_manager: &mut DisplayManager,
) -> DisplayId {
    let Some(uuid) = display_manager_arrangement_display_uuid(arrangement, display_manager) else {
        return DisplayId(0);
    };

    display_id(uuid.as_ref())
}

pub(crate) fn display_manager_cursor_display_id() -> DisplayId {
    let connection_id = *CONNECTION.get().unwrap();
    let mut cursor = CGPoint::ZERO;
    unsafe { SLSGetCurrentCursorLocation(connection_id, &mut cursor) };
    display_manager_point_display_id(cursor)
}

pub(crate) fn display_manager_prev_display_id(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
) -> DisplayId {
    let arrangement = display_manager_display_id_arrangement(display_id, display_manager);
    if arrangement <= 1 {
        return DisplayId(0);
    }

    display_manager_arrangement_display_id(arrangement - 1, display_manager)
}

pub(crate) fn display_manager_next_display_id(
    display_id: DisplayId,
    display_manager: &mut DisplayManager,
) -> DisplayId {
    let arrangement = display_manager_display_id_arrangement(display_id, display_manager);
    if arrangement >= display_manager_active_display_count() {
        return DisplayId(0);
    }

    display_manager_arrangement_display_id(arrangement + 1, display_manager)
}

pub(crate) fn display_manager_first_display_id(display_manager: &mut DisplayManager) -> DisplayId {
    display_manager_arrangement_display_id(1, display_manager)
}

pub(crate) fn display_manager_last_display_id(display_manager: &mut DisplayManager) -> DisplayId {
    let arrangement = display_manager_active_display_count();
    display_manager_arrangement_display_id(arrangement, display_manager)
}

pub(crate) fn display_manager_find_closest_display_in_direction(
    source_display_id: DisplayId,
    direction: i32,
) -> DisplayId {
    let display_list = display_manager_active_display_list();

    let mut best_display_id = DisplayId(0);
    let mut best_distance = i32::MAX;

    let source_area = area_from_cgrect(CGDisplayBounds(source_display_id.0));
    let source_area_max_point = area_max_point(source_area);

    let display_count = display_list.len() as i32;
    for index in 0..display_count {
        let display_id = display_list[index as usize];
        if display_id == source_display_id {
            continue;
        }

        let target_area = area_from_cgrect(CGDisplayBounds(display_id.0));
        let target_area_max_point = area_max_point(target_area);

        if area_is_in_direction(
            &source_area,
            source_area_max_point,
            &target_area,
            target_area_max_point,
            direction,
        ) {
            let distance = area_distance_in_direction(
                &source_area,
                source_area_max_point,
                &target_area,
                target_area_max_point,
                direction,
            );
            if distance < best_distance {
                best_display_id = display_id;
                best_distance = distance;
            }
        }
    }

    best_display_id
}

pub(crate) fn display_manager_menu_bar_hidden() -> bool {
    let connection_id = *CONNECTION.get().unwrap();
    let mut status: c_int = 0;
    unsafe { SLSGetMenuBarAutohideEnabled(connection_id, &mut status) };
    status != 0
}

pub(crate) fn display_manager_menu_bar_rect(display_id: DisplayId) -> CGRect {
    let mut bounds = CGRect::ZERO;

    #[cfg(target_arch = "x86_64")]
    {
        let connection_id = *CONNECTION.get().unwrap();
        unsafe {
            SLSGetRevealedMenuBarBounds(
                &mut bounds,
                connection_id,
                display_space_id(display_id).0,
            )
        };
    }
    #[cfg(target_arch = "aarch64")]
    {
        //
        // NOTE(asmvik): SLSGetRevealedMenuBarBounds is broken on Apple Silicon,
        // but we expected it to return the full display bounds along with the menubar
        // height. Combine this information ourselves using two separate functions..
        //

        let mut height: u32 = 0;
        unsafe { SLSGetDisplayMenubarHeight(display_id.0, &mut height) };

        bounds = CGDisplayBounds(display_id.0);
        bounds.size.height = height as CGFloat;
    }

    //
    // NOTE(asmvik): Height needs to be offset by 1 because that is the actual
    // position on the screen that windows can be positioned at..
    //

    bounds.size.height += 1 as CGFloat;
    bounds
}

pub(crate) fn display_manager_dock_hidden() -> bool {
    let auto_hide_enabled = unsafe { CoreDockGetAutoHideEnabled() };
    auto_hide_enabled != 0
}

pub(crate) fn display_manager_dock_orientation() -> i32 {
    let mut pinning: c_int = 0;
    let mut orientation: c_int = 0;
    unsafe { CoreDockGetOrientationAndPinning(&mut orientation, &mut pinning) };
    orientation
}

pub(crate) fn display_manager_dock_rect() -> CGRect {
    let connection_id = *CONNECTION.get().unwrap();
    let mut reason: c_int = 0;
    let mut bounds = CGRect::ZERO;
    unsafe { SLSGetDockRectWithReason(connection_id, &mut bounds, &mut reason) };
    bounds
}

pub(crate) fn display_manager_display_is_animating(display_id: DisplayId) -> bool {
    if workspace_is_macos_bigsur()
        || workspace_is_macos_monterey()
        || workspace_is_macos_ventura()
    {
        let connection_id = *CONNECTION.get().unwrap();

        let Some(uuid) = display_uuid(display_id) else {
            return false;
        };

        let result = unsafe { SLSManagedDisplayIsAnimating(connection_id, uuid.as_ref()) };

        return result;
    }

    false // This does not return a correct result on modern macOS versions.
}

pub(crate) fn display_manager_active_display_count() -> i32 {
    let mut count: u32 = 0;
    unsafe { CGGetActiveDisplayList(0, core::ptr::null_mut(), &mut count) };
    count as i32
}

pub(crate) fn display_manager_active_display_list() -> Vec<DisplayId> {
    let display_count = display_manager_active_display_count();
    let mut result: Vec<u32> = vec![0; display_count as usize];
    let mut count: u32 = 0;
    unsafe { CGGetActiveDisplayList(display_count as u32, result.as_mut_ptr(), &mut count) };
    result.truncate(count as usize);
    result.into_iter().map(DisplayId).collect()
}

pub(crate) fn display_manager_find_element_at_point(
    point: CGPoint,
    window_manager: &mut WindowManager,
) -> Option<CFRetained<AXUIElement>> {
    let mut element_ref: *const AXUIElement = core::ptr::null();
    unsafe {
        AXUIElementCopyElementAtPosition(
            &*window_manager.system_element,
            point.x as f32,
            point.y as f32,
            NonNull::from(&mut element_ref),
        )
    };
    let element_ref = unsafe { take_create_rule_result(element_ref) }?;

    let mut role: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &element_ref,
            kAXRoleAttribute(),
            NonNull::from(&mut role),
        )
    };
    let role = unsafe { take_create_rule_result(role) }?;

    if CFEqual(Some(&role), Some(as_cftype(kAXWindowRole()))) {
        return Some(element_ref);
    }

    let mut window_ref: *const CFType = core::ptr::null();
    unsafe {
        AXUIElementCopyAttributeValue(
            &element_ref,
            kAXWindowAttribute(),
            NonNull::from(&mut window_ref),
        )
    };
    let window_ref = unsafe { take_create_rule_result(window_ref.cast::<AXUIElement>()) };
    drop(element_ref);
    drop(role);
    window_ref
}

pub(crate) fn display_manager_focus_display_with_window_at_point(
    point: CGPoint,
    window_manager: &mut WindowManager,
) -> WindowId {
    let connection_id = *CONNECTION.get().unwrap();

    let mut element_connection: c_int = 0;
    let mut element_process_serial_number = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };

    let Some(element_ref) = display_manager_find_element_at_point(point, window_manager) else {
        return WindowId(0);
    };

    let element_id = ax_window_id(&element_ref);
    if element_id == 0 {
        return WindowId(0);
    }

    unsafe { SLSGetWindowOwner(connection_id, element_id, &mut element_connection) };
    unsafe { SLSGetConnectionPSN(element_connection, &mut element_process_serial_number) };
    window_manager_focus_window_with_raise(
        &element_process_serial_number,
        WindowId(element_id),
        &*element_ref,
    );
    WindowId(element_id)
}

pub(crate) fn display_manager_set_active_display_id(display_id: DisplayId) {
    let connection_id = *CONNECTION.get().unwrap();
    let Some(uuid) = display_uuid(display_id) else {
        return;
    };
    unsafe { SLSSetActiveMenuBarDisplayIdentifier(connection_id, uuid.as_ref(), uuid.as_ref()) };
}

pub(crate) fn display_manager_focus_display(
    display_id: DisplayId,
    space_id: SpaceId,
    window_manager: &mut WindowManager,
) {
    let window_id = window_manager_find_window_on_space_by_rank_filtering_window(
        window_manager,
        space_id,
        1,
        WindowId(0),
    );
    if let Some(window_id) = window_id {
        let Some(window) = window_manager.window.find(&window_id) else {
            return;
        };
        let window_element_ref = window.element_ref;
        let Some(window_process_id) = window.application else {
            return;
        };
        let Some(application) = window_manager.application.find(&window_process_id) else {
            return;
        };
        let window_process_serial_number = application.process_serial_number;

        window_manager_focus_window_with_raise(
            &window_process_serial_number,
            window_id,
            window_element_ref,
        );
        window_manager_center_mouse(window_manager, window_id);
        display_manager_set_active_display_id(display_id);
    } else {
        let point = display_center(display_id);
        CGWarpMouseCursorPosition(point);
        display_manager_set_active_display_id(display_id);

        if space_manager_active_space(window_manager) != display_space_id(display_id) {
            unsafe { CGPostMouseEvent(point, 0, 1, 1) };
            unsafe { CGPostMouseEvent(point, 0, 1, 0) };
        }
    }
}

pub(crate) fn display_manager_focus_space(
    display_id: DisplayId,
    space_id: SpaceId,
    mission_control_mode: &mut MissionControlMode,
) -> SpaceOpError {
    let is_in_mission_control = mission_control_is_active(mission_control_mode);
    if is_in_mission_control {
        return SpaceOpError::InMissionControl;
    }

    let is_animating = display_manager_display_is_animating(display_id);
    if is_animating {
        return SpaceOpError::DisplayIsAnimating;
    }

    let space_display_id = space_display_id(space_id);
    if space_display_id != display_id {
        return SpaceOpError::SameDisplay;
    }

    if scripting_addition_focus_space(space_id) {
        SpaceOpError::Success
    } else {
        SpaceOpError::ScriptingAddition
    }
}

pub(crate) fn display_manager_begin(display_manager: &mut DisplayManager) -> bool {
    display_manager.current_display_id = display_manager_active_display_id();
    display_manager.last_display_id = display_manager.current_display_id;
    display_manager.order = DisplayArrangementOrder::Default;
    display_manager.mode = ExternalBarMode::Off;
    display_manager.top_padding = 0;
    display_manager.bottom_padding = 0;
    let registration_result = unsafe {
        CGDisplayRegisterReconfigurationCallback(Some(display_handler), core::ptr::null_mut())
    };
    registration_result == kCGErrorSuccess
}
