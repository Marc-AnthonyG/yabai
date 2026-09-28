use core::ffi::CStr;
use std::fs::{DirBuilder, Permissions};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use objc2::rc::autoreleasepool;
use objc2::sel;

use crate::ffi::appkit::NSRunningApplication;
use crate::ffi::foundation::{NSBundle, NSString};
use crate::ffi::libsystem::{
    CSR_ALLOW_TASK_FOR_PID, CSR_ALLOW_UNRESTRICTED_FS, csr_get_active_config,
};
use crate::scripting_addition::client::request_scripting_addition_handshake;
use crate::scripting_addition::frame::{
    SCRIPTING_ADDITION_FOUND_EVERYTHING_IT_NEEDS, SCRIPTING_ADDITION_SOCKET_PATH_FORMAT,
    SCRIPTING_ADDITION_VERSION,
};
use crate::state::process_wide::SCRIPTING_ADDITION_SOCKET_PATH;
use crate::support::privilege::is_running_as_root;
use crate::support::strings::are_both_strings_present_and_equal;
use crate::{notify, warn};

pub(crate) static SCRIPTING_ADDITION_PAYLOAD_BINARY: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/payload"));
pub(crate) static SCRIPTING_ADDITION_LOADER_BINARY: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/loader"));

pub(crate) struct ScriptingAdditionBundlePaths {
    pub(crate) base_directory: String,
    pub(crate) contents_directory: String,
    pub(crate) contents_macos_directory: String,
    pub(crate) contents_resources_directory: String,
    pub(crate) info_plist: String,
    pub(crate) payload_directory: String,
    pub(crate) payload_contents_directory: String,
    pub(crate) payload_contents_macos_directory: String,
    pub(crate) payload_plist: String,
    pub(crate) binary_payload: String,
    pub(crate) binary_loader: String,
}

static SCRIPTING_ADDITION_BUNDLE_PATHS: OnceLock<ScriptingAdditionBundlePaths> = OnceLock::new();

pub(crate) const SCRIPTING_ADDITION_INFO_PLIST_TEMPLATE: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
    "<plist version=\"1.0\">\n",
    "<dict>\n",
    "<key>CFBundleDevelopmentRegion</key>\n",
    "<string>en</string>\n",
    "<key>CFBundleExecutable</key>\n",
    "<string>loader</string>\n",
    "<key>CFBundleIdentifier</key>\n",
    "<string>com.asmvik.yabai-osax</string>\n",
    "<key>CFBundleInfoDictionaryVersion</key>\n",
    "<string>6.0</string>\n",
    "<key>CFBundleName</key>\n",
    "<string>yabai</string>\n",
    "<key>CFBundlePackageType</key>\n",
    "<string>osax</string>\n",
    "<key>CFBundleShortVersionString</key>\n",
    "<string>{}</string>\n",
    "<key>CFBundleVersion</key>\n",
    "<string>{}</string>\n",
    "<key>NSHumanReadableCopyright</key>\n",
    "<string>Copyright © 2019 Åsmund Vikane. All rights reserved.</string>\n",
    "<key>OSAXHandlers</key>\n",
    "<dict>\n",
    "</dict>\n",
    "</dict>\n",
    "</plist>",
);

pub(crate) const SCRIPTING_ADDITION_PAYLOAD_INFO_PLIST_TEMPLATE: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
    "<plist version=\"1.0\">\n",
    "<dict>\n",
    "<key>CFBundleDevelopmentRegion</key>\n",
    "<string>en</string>\n",
    "<key>CFBundleExecutable</key>\n",
    "<string>payload</string>\n",
    "<key>CFBundleIdentifier</key>\n",
    "<string>com.asmvik.yabai-sa</string>\n",
    "<key>CFBundleInfoDictionaryVersion</key>\n",
    "<string>6.0</string>\n",
    "<key>CFBundleName</key>\n",
    "<string>payload</string>\n",
    "<key>CFBundlePackageType</key>\n",
    "<string>BNDL</string>\n",
    "<key>CFBundleShortVersionString</key>\n",
    "<string>{}</string>\n",
    "<key>CFBundleVersion</key>\n",
    "<string>{}</string>\n",
    "<key>NSHumanReadableCopyright</key>\n",
    "<string>Copyright © 2019 Åsmund Vikane. All rights reserved.</string>\n",
    "<key>NSPrincipalClass</key>\n",
    "<string></string>\n",
    "</dict>\n",
    "</plist>",
);

pub(crate) fn build_scripting_addition_bundle_paths() -> ScriptingAdditionBundlePaths {
    let base_directory = String::from("/Library/ScriptingAdditions/yabai.osax");
    let contents_directory = format!("{base_directory}/Contents");
    let contents_macos_directory = format!("{contents_directory}/MacOS");
    let contents_resources_directory = format!("{contents_directory}/Resources");
    let info_plist = format!("{contents_directory}/Info.plist");
    let payload_directory = format!("{contents_resources_directory}/payload.bundle");
    let payload_contents_directory = format!("{payload_directory}/Contents");
    let payload_contents_macos_directory = format!("{payload_contents_directory}/MacOS");
    let payload_plist = format!("{payload_contents_directory}/Info.plist");
    let binary_loader = format!("{contents_macos_directory}/loader");
    let binary_payload = format!("{payload_contents_macos_directory}/payload");

    ScriptingAdditionBundlePaths {
        base_directory,
        contents_directory,
        contents_macos_directory,
        contents_resources_directory,
        info_plist,
        payload_directory,
        payload_contents_directory,
        payload_contents_macos_directory,
        payload_plist,
        binary_payload,
        binary_loader,
    }
}

fn scripting_addition_bundle_paths() -> &'static ScriptingAdditionBundlePaths {
    SCRIPTING_ADDITION_BUNDLE_PATHS.get_or_init(build_scripting_addition_bundle_paths)
}

pub(crate) fn create_scripting_addition_bundle_directories() -> bool {
    let osax_paths = scripting_addition_bundle_paths();
    [
        &osax_paths.base_directory,
        &osax_paths.contents_directory,
        &osax_paths.contents_macos_directory,
        &osax_paths.contents_resources_directory,
        &osax_paths.payload_directory,
        &osax_paths.payload_contents_directory,
        &osax_paths.payload_contents_macos_directory,
    ]
    .into_iter()
    .all(|directory| DirBuilder::new().mode(0o755).create(directory).is_ok())
}

pub(crate) fn make_scripting_addition_binaries_executable_and_ad_hoc_signed() {
    let osax_paths = scripting_addition_bundle_paths();
    for binary in [&osax_paths.binary_loader, &osax_paths.binary_payload] {
        let _ = std::fs::set_permissions(binary, Permissions::from_mode(0o755));
        let _ = Command::new("codesign")
            .args(["-f", "-s", "-"])
            .arg(binary)
            .stderr(Stdio::null())
            .status();
    }
}

pub(crate) fn terminate_dock_so_it_restarts() {
    let dock = NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
        "com.apple.dock",
    ));
    unsafe { dock.makeObjectsPerformSelector(sel!(terminate)) };
}

pub(crate) fn store_scripting_addition_socket_path_of_sudo_user() -> bool {
    let Some(sudo_user_id) = std::env::var("SUDO_UID")
        .ok()
        .and_then(|user_id| user_id.parse::<libc::uid_t>().ok())
    else {
        return false;
    };

    let password_entry = unsafe { libc::getpwuid(sudo_user_id) };
    if password_entry.is_null() {
        return false;
    }

    let user_name = unsafe { CStr::from_ptr((*password_entry).pw_name) }.to_string_lossy();
    let _ = SCRIPTING_ADDITION_SOCKET_PATH
        .set(SCRIPTING_ADDITION_SOCKET_PATH_FORMAT.replacen("%s", &user_name, 1));
    true
}

pub(crate) fn is_scripting_addition_installed() -> bool {
    Path::new(&scripting_addition_bundle_paths().base_directory).is_dir()
}

pub(crate) fn is_scripting_addition_missing_or_outdated() -> bool {
    if !is_scripting_addition_installed() {
        return true;
    }

    autoreleasepool(|_pool| {
        let payload_path = NSString::from_str(&scripting_addition_bundle_paths().payload_directory);
        let installed_version = NSBundle::bundleWithPath(&payload_path)
            .and_then(|payload_bundle| {
                payload_bundle.objectForInfoDictionaryKey(&NSString::from_str("CFBundleVersion"))
            })
            .and_then(|version| version.downcast::<NSString>().ok())
            .map(|version| version.to_string());
        installed_version.as_deref() != Some(SCRIPTING_ADDITION_VERSION)
    })
}

pub(crate) fn remove_scripting_addition_bundle() -> bool {
    std::fs::remove_dir_all(&scripting_addition_bundle_paths().base_directory).is_ok()
}

fn write_scripting_addition_bundle() -> bool {
    let osax_paths = scripting_addition_bundle_paths();
    let info_plist =
        SCRIPTING_ADDITION_INFO_PLIST_TEMPLATE.replace("{}", SCRIPTING_ADDITION_VERSION);
    let payload_plist =
        SCRIPTING_ADDITION_PAYLOAD_INFO_PLIST_TEMPLATE.replace("{}", SCRIPTING_ADDITION_VERSION);

    create_scripting_addition_bundle_directories()
        && std::fs::write(&osax_paths.info_plist, info_plist).is_ok()
        && std::fs::write(&osax_paths.payload_plist, payload_plist).is_ok()
        && std::fs::write(&osax_paths.binary_loader, SCRIPTING_ADDITION_LOADER_BINARY).is_ok()
        && std::fs::write(
            &osax_paths.binary_payload,
            SCRIPTING_ADDITION_PAYLOAD_BINARY,
        )
        .is_ok()
}

pub(crate) fn install_scripting_addition_bundle_and_restart_dock() -> i32 {
    unsafe { libc::umask(libc::S_IWGRP | libc::S_IWOTH) };

    if is_scripting_addition_installed() && !remove_scripting_addition_bundle() {
        return 1;
    }

    if !write_scripting_addition_bundle() {
        remove_scripting_addition_bundle();
        return 2;
    }

    make_scripting_addition_binaries_executable_and_ad_hoc_signed();
    terminate_dock_so_it_restarts();
    0
}

pub(crate) fn validate_loaded_scripting_addition_updating_it_if_outdated() -> i32 {
    let mut attributes: u32 = 0;
    let mut version = String::new();
    let is_latest_version_installed = !is_scripting_addition_missing_or_outdated();

    if !request_scripting_addition_handshake(&mut version, &mut attributes) {
        notify!("scripting-addition", "connection failed!");
        return 1;
    }

    if are_both_strings_present_and_equal(Some(&version), Some(SCRIPTING_ADDITION_VERSION)) {
        if (attributes & SCRIPTING_ADDITION_FOUND_EVERYTHING_IT_NEEDS)
            == SCRIPTING_ADDITION_FOUND_EVERYTHING_IT_NEEDS
        {
            notify!("scripting-addition", "payload v{}", version);
            return 0;
        }

        notify!(
            "scripting-addition",
            "payload (0x{:X}) doesn't support this macOS version!",
            attributes
        );
        return 1;
    }

    if !is_latest_version_installed {
        notify!("scripting-addition", "payload is outdated, updating..");
        return install_scripting_addition_bundle_and_restart_dock();
    }

    notify!(
        "scripting-addition",
        "payload is outdated, restarting Dock.app.."
    );
    terminate_dock_so_it_restarts();
    0
}

pub(crate) fn is_system_integrity_protection_relaxed_enough_for_scripting_addition() -> bool {
    let mut configuration: u32 = 0;
    unsafe { csr_get_active_config(&mut configuration) };

    if (configuration & CSR_ALLOW_UNRESTRICTED_FS) == 0 {
        return false;
    }

    if (configuration & CSR_ALLOW_TASK_FOR_PID) == 0 {
        return false;
    }

    true
}

#[cfg(target_arch = "aarch64")]
pub(crate) fn is_arm64e_preview_abi_boot_argument_set() -> bool {
    let mut boot_arguments = [0u8; 2048];
    let mut length: usize = boot_arguments.len() - 1;

    if unsafe {
        libc::sysctlbyname(
            c"kern.bootargs".as_ptr(),
            boot_arguments.as_mut_ptr().cast(),
            &mut length,
            core::ptr::null_mut(),
            0,
        )
    } == 0
    {
        let searched_boot_arguments = &boot_arguments[..length];
        let searched_boot_arguments = match searched_boot_arguments
            .iter()
            .position(|&byte| byte == b'\0')
        {
            Some(terminator) => &searched_boot_arguments[..terminator],
            None => searched_boot_arguments,
        };
        if searched_boot_arguments
            .windows(b"-arm64e_preview_abi".len())
            .any(|window| window == b"-arm64e_preview_abi")
        {
            return true;
        }
    }

    false
}

pub(crate) fn run_loader_to_inject_payload_into_dock() -> bool {
    Command::new(&scripting_addition_bundle_paths().binary_loader)
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|exit_status| exit_status.success())
}

pub(crate) fn uninstall_scripting_addition() -> i32 {
    if !is_system_integrity_protection_relaxed_enough_for_scripting_addition() {
        warn!(
            "yabai: System Integrity Protection: Filesystem Protections and Debugging Restrictions must be disabled!\n"
        );
        notify!(
            "scripting-addition",
            "System Integrity Protection: Filesystem Protections and Debugging Restrictions must be disabled!"
        );
        return 1;
    }

    if !is_running_as_root() {
        warn!("yabai: scripting-addition must be uninstalled as root!\n");
        notify!("scripting-addition", "must be uninstalled as root!");
        return 1;
    }

    if !is_scripting_addition_installed() {
        return 0;
    }
    if !remove_scripting_addition_bundle() {
        return -1;
    }
    0
}

pub(crate) fn install_and_load_scripting_addition() -> i32 {
    autoreleasepool(|_pool| {
        let mut result = 0;

        if !is_running_as_root() {
            warn!("yabai: scripting-addition must be loaded as root!\n");
            notify!("scripting-addition", "must be loaded as root!");
            result = 1;
            return result;
        }

        if !is_system_integrity_protection_relaxed_enough_for_scripting_addition() {
            warn!(
                "yabai: System Integrity Protection: Filesystem Protections and Debugging Restrictions must be disabled!\n"
            );
            notify!(
                "scripting-addition",
                "System Integrity Protection: Filesystem Protections and Debugging Restrictions must be disabled!"
            );
            result = 1;
            return result;
        }

        if is_scripting_addition_missing_or_outdated() {
            result = install_scripting_addition_bundle_and_restart_dock();
            return result;
        }

        #[cfg(target_arch = "aarch64")]
        {
            if !is_arm64e_preview_abi_boot_argument_set() {
                warn!("yabai: missing required nvram boot-arg '-arm64e_preview_abi'!\n");
                notify!(
                    "scripting-addition",
                    "missing required nvram boot-arg '-arm64e_preview_abi'!"
                );
                result = 1;
                return result;
            }
        }

        if !run_loader_to_inject_payload_into_dock() {
            warn!("yabai: scripting-addition failed to inject payload into Dock.app!\n");
            notify!(
                "scripting-addition",
                "failed to inject payload into Dock.app!"
            );
            result = 1;
            return result;
        }

        if store_scripting_addition_socket_path_of_sudo_user() {
            result = validate_loaded_scripting_addition_updating_it_if_outdated();
        }

        result
    })
}
