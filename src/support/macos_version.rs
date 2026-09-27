use std::sync::atomic::{AtomicBool, Ordering};

use crate::ffi::foundation::NSProcessInfo;

macro_rules! supported_macos_version_list {
    ($entry:ident) => {
        $entry!(tahoe,    _workspace_is_macos_version_tahoe,    workspace_is_macos_tahoe,    26);
        $entry!(sequoia,  _workspace_is_macos_version_sequoia,  workspace_is_macos_sequoia,  15);
        $entry!(sonoma,   _workspace_is_macos_version_sonoma,   workspace_is_macos_sonoma,   14);
        $entry!(ventura,  _workspace_is_macos_version_ventura,  workspace_is_macos_ventura,  13);
        $entry!(monterey, _workspace_is_macos_version_monterey, workspace_is_macos_monterey, 12);
        $entry!(bigsur,   _workspace_is_macos_version_bigsur,   workspace_is_macos_bigsur,   11);
    };
}

macro_rules! support_macos_version {
    ($name:ident, $flag_name:ident, $accessor_name:ident, $major_version:literal) => {
        #[allow(non_upper_case_globals)]
        pub(crate) static $flag_name: AtomicBool = AtomicBool::new(false);

        pub(crate) fn $accessor_name() -> bool {
            $flag_name.load(Ordering::Relaxed)
        }
    };
}

supported_macos_version_list!(support_macos_version);

pub(crate) use supported_macos_version_list;

pub(crate) fn workspace_use_macos_space_workaround() -> bool {
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
