use core::ffi::{c_int, c_uint};

use objc2::msg_send;

use crate::display::identity::{display_manager_dock_display_id, display_manager_main_display_id};
use crate::display::manager::{DisplayManager, ExternalBarMode};
use crate::ffi::appkit::NSScreen;
use crate::ffi::carbon_process::{CoreDockGetAutoHideEnabled, CoreDockGetOrientationAndPinning};
use crate::ffi::core_foundation::{CGFloat, CGPoint, CGRect};
use crate::ffi::core_graphics::{CGDisplayBounds, CGDisplayIsBuiltin};
use crate::ffi::foundation::{MainThreadMarker, NSString};
use crate::ffi::skylight::{SLSGetDockRectWithReason, SLSGetMenuBarAutohideEnabled};
use crate::state::process_wide::CONNECTION;
use crate::support::handles::DisplayId;
use crate::support::macos_version::workspace_is_macos_bigsur;

#[cfg(target_arch = "aarch64")]
use crate::ffi::skylight::SLSGetDisplayMenubarHeight;
#[cfg(target_arch = "x86_64")]
use crate::display::spaces::display_space_id;
#[cfg(target_arch = "x86_64")]
use crate::ffi::skylight::SLSGetRevealedMenuBarBounds;

pub(crate) const DOCK_ORIENTATION_BOTTOM: i32 = 2;
pub(crate) const DOCK_ORIENTATION_LEFT: i32 = 3;
pub(crate) const DOCK_ORIENTATION_RIGHT: i32 = 4;

pub(crate) fn display_bounds_constrained(
    display_id: DisplayId,
    ignore_external_bar: bool,
    display_manager: &mut DisplayManager,
) -> CGRect {
    let mut frame = CGDisplayBounds(display_id.0);
    let mut effective_external_top_padding: i32 = 0;

    if !ignore_external_bar {
        if (display_manager.mode == ExternalBarMode::Main
            && display_id == display_manager_main_display_id())
            || (display_manager.mode == ExternalBarMode::All)
        {
            effective_external_top_padding = display_manager.top_padding;

            frame.origin.y += effective_external_top_padding as f64;
            frame.size.height -= effective_external_top_padding as f64;
            frame.size.height -= display_manager.bottom_padding as f64;
        }
    }

    if display_manager_menu_bar_hidden() {
        let notch_height = workspace_display_notch_height(display_id);
        if notch_height > effective_external_top_padding {
            frame.origin.y += (notch_height - effective_external_top_padding) as f64;
            frame.size.height -= (notch_height - effective_external_top_padding) as f64;
        }
    } else {
        let menu = display_manager_menu_bar_rect(display_id);
        frame.origin.y += menu.size.height;
        frame.size.height -= menu.size.height;
    }

    if !display_manager_dock_hidden() {
        if display_id == display_manager_dock_display_id() {
            let dock = display_manager_dock_rect();
            match display_manager_dock_orientation() {
                DOCK_ORIENTATION_LEFT => {
                    frame.origin.x += dock.size.width;
                    frame.size.width -= dock.size.width;
                }
                DOCK_ORIENTATION_RIGHT => {
                    frame.size.width -= dock.size.width;
                }
                DOCK_ORIENTATION_BOTTOM => {
                    frame.size.height -= dock.size.height;
                }
                _ => {}
            }
        }
    }

    frame
}

pub(crate) fn display_center(display_id: DisplayId) -> CGPoint {
    let bounds = CGDisplayBounds(display_id.0);
    CGPoint {
        x: bounds.origin.x + bounds.size.width / 2.0,
        y: bounds.origin.y + bounds.size.height / 2.0,
    }
}

pub(crate) fn display_manager_menu_bar_hidden() -> bool {
    let connection_id = *CONNECTION.get().unwrap();
    let mut status: c_int = 0;
    unsafe { SLSGetMenuBarAutohideEnabled(connection_id, &mut status) };
    status != 0
}

pub(crate) fn display_manager_menu_bar_rect(display_id: DisplayId) -> CGRect {
    #[cfg(target_arch = "x86_64")]
    let mut bounds = CGRect::ZERO;
    #[cfg(target_arch = "aarch64")]
    let mut bounds: CGRect;

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

pub(crate) fn workspace_display_notch_height(display_id: DisplayId) -> i32 {
    if !CGDisplayIsBuiltin(display_id.0) {
        return 0;
    }

    if !workspace_is_macos_bigsur() {
        let screen_list = unsafe {
            let main_thread_marker = MainThreadMarker::new_unchecked();
            NSScreen::screens(main_thread_marker)
        };

        for screen in screen_list.iter() {
            let screen_number: c_uint = screen
                .deviceDescription()
                .objectForKey(&NSString::from_str("NSScreenNumber"))
                .map_or(0, |screen_number| unsafe {
                    msg_send![&*screen_number, unsignedIntValue]
                });
            if screen_number == display_id.0 {
                return screen.safeAreaInsets().top as i32;
            }
        }
    }

    0
}
