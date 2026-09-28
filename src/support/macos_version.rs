use std::sync::atomic::{AtomicBool, Ordering};

use crate::ffi::foundation::NSProcessInfo;

macro_rules! with_every_supported_macos_version {
    ($entry:ident) => {
        $entry!(tahoe, RUNNING_ON_MACOS_TAHOE, is_running_on_macos_tahoe, 26);
        $entry!(
            sequoia,
            RUNNING_ON_MACOS_SEQUOIA,
            is_running_on_macos_sequoia,
            15
        );
        $entry!(
            sonoma,
            RUNNING_ON_MACOS_SONOMA,
            is_running_on_macos_sonoma,
            14
        );
        $entry!(
            ventura,
            RUNNING_ON_MACOS_VENTURA,
            is_running_on_macos_ventura,
            13
        );
        $entry!(
            monterey,
            RUNNING_ON_MACOS_MONTEREY,
            is_running_on_macos_monterey,
            12
        );
        $entry!(
            bigsur,
            RUNNING_ON_MACOS_BIG_SUR,
            is_running_on_macos_big_sur,
            11
        );
    };
}

macro_rules! define_running_on_macos_version_flag_and_accessor {
    ($name:ident, $flag_name:ident, $accessor_name:ident, $major_version:literal) => {
        pub(crate) static $flag_name: AtomicBool = AtomicBool::new(false);

        pub(crate) fn $accessor_name() -> bool {
            $flag_name.load(Ordering::Relaxed)
        }
    };
}

with_every_supported_macos_version!(define_running_on_macos_version_flag_and_accessor);

pub(crate) use with_every_supported_macos_version;

pub(crate) fn is_workaround_needed_to_move_windows_between_spaces() -> bool {
    let os_version = NSProcessInfo::processInfo().operatingSystemVersion();

    if os_version.majorVersion == 12 && os_version.minorVersion >= 7 {
        return true;
    }
    if os_version.majorVersion == 13 && os_version.minorVersion >= 6 {
        return true;
    }
    if os_version.majorVersion == 14 && os_version.minorVersion >= 5 {
        return true;
    }

    os_version.majorVersion >= 15
}
