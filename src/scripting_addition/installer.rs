use core::ffi::{CStr, c_char, c_void};
use std::ffi::CString;
use std::sync::OnceLock;

use objc2::rc::autoreleasepool;
use objc2::{msg_send, sel};

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
use crate::support::strings::{FIXED_STRING_BUFFER_LENGTH, are_both_strings_present_and_equal};
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

fn truncate_as_snprintf_would_into_a_fixed_string_buffer(mut text: String) -> String {
    let mut length = text.len().min(FIXED_STRING_BUFFER_LENGTH - 1);
    while !text.is_char_boundary(length) {
        length -= 1;
    }
    text.truncate(length);
    text
}

pub(crate) fn build_scripting_addition_bundle_paths() -> ScriptingAdditionBundlePaths {
    let base_directory = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{}",
        "/Library/ScriptingAdditions/yabai.osax"
    ));

    let contents_directory = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{}/{}",
        base_directory, "Contents"
    ));
    let contents_macos_directory = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{}/{}",
        contents_directory, "MacOS"
    ));
    let contents_resources_directory = truncate_as_snprintf_would_into_a_fixed_string_buffer(
        format!("{}/{}", contents_directory, "Resources"),
    );
    let info_plist = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{}/{}",
        contents_directory, "Info.plist"
    ));

    let payload_directory = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{}/{}",
        contents_resources_directory, "payload.bundle"
    ));
    let payload_contents_directory = truncate_as_snprintf_would_into_a_fixed_string_buffer(
        format!("{}/{}", payload_directory, "Contents"),
    );
    let payload_contents_macos_directory = truncate_as_snprintf_would_into_a_fixed_string_buffer(
        format!("{}/{}", payload_contents_directory, "MacOS"),
    );
    let payload_plist = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{}/{}",
        payload_contents_directory, "Info.plist"
    ));

    let binary_loader = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{}/{}",
        contents_macos_directory, "loader"
    ));
    let binary_payload = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{}/{}",
        payload_contents_macos_directory, "payload"
    ));

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

pub(crate) fn create_scripting_addition_bundle_directories() -> bool {
    let osax_paths =
        SCRIPTING_ADDITION_BUNDLE_PATHS.get_or_init(build_scripting_addition_bundle_paths);
    let directory_list = [
        &osax_paths.base_directory,
        &osax_paths.contents_directory,
        &osax_paths.contents_macos_directory,
        &osax_paths.contents_resources_directory,
        &osax_paths.payload_directory,
        &osax_paths.payload_contents_directory,
        &osax_paths.payload_contents_macos_directory,
    ];

    for directory in directory_list {
        let directory = CString::new(directory.as_str()).unwrap();
        if unsafe { libc::mkdir(directory.as_ptr(), 0o755) } != 0 {
            return false;
        }
    }

    true
}

pub(crate) fn write_bytes_to_file_opened_with_mode(
    buffer: &[u8],
    file: &str,
    file_mode: &str,
) -> bool {
    let file = CString::new(file).unwrap();
    let file_mode = CString::new(file_mode).unwrap();

    let handle = unsafe { libc::fopen(file.as_ptr(), file_mode.as_ptr()) };
    if handle.is_null() {
        return false;
    }

    let bytes = unsafe { libc::fwrite(buffer.as_ptr().cast::<c_void>(), buffer.len(), 1, handle) };
    let result = bytes == 1;
    unsafe { libc::fclose(handle) };

    result
}

pub(crate) fn make_scripting_addition_binaries_executable_and_ad_hoc_signed() {
    let osax_paths =
        SCRIPTING_ADDITION_BUNDLE_PATHS.get_or_init(build_scripting_addition_bundle_paths);

    let command = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{} {}",
        "chmod +x", osax_paths.binary_loader
    ));
    unsafe { libc::system(CString::new(command).unwrap().as_ptr()) };

    let command = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{} {} {}",
        "codesign -f -s -", osax_paths.binary_loader, "2>/dev/null"
    ));
    unsafe { libc::system(CString::new(command).unwrap().as_ptr()) };

    let command = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{} {}",
        "chmod +x", osax_paths.binary_payload
    ));
    unsafe { libc::system(CString::new(command).unwrap().as_ptr()) };

    let command = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{} {} {}",
        "codesign -f -s -", osax_paths.binary_payload, "2>/dev/null"
    ));
    unsafe { libc::system(CString::new(command).unwrap().as_ptr()) };
}

pub(crate) fn terminate_dock_so_it_restarts() {
    let dock =
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
            "com.apple.dock",
        ));
    unsafe { dock.makeObjectsPerformSelector(sel!(terminate)) };
}

pub(crate) fn store_scripting_addition_socket_path_of_sudo_user() -> bool {
    let sudo_user_id = unsafe { libc::getenv(c"SUDO_UID".as_ptr()) };

    let mut user_id: libc::uid_t = unsafe { libc::getuid() };
    debug_assert!(user_id == 0);

    if sudo_user_id.is_null() {
        return false;
    }
    if unsafe { libc::sscanf(sudo_user_id, c"%u".as_ptr(), &mut user_id as *mut libc::uid_t) } != 1
    {
        return false;
    }

    let password_entry = unsafe { libc::getpwuid(user_id) };
    if password_entry.is_null() {
        return false;
    }

    let user_name = unsafe { CStr::from_ptr((*password_entry).pw_name) }.to_string_lossy();
    let _ =
        SCRIPTING_ADDITION_SOCKET_PATH.set(truncate_as_snprintf_would_into_a_fixed_string_buffer(
            SCRIPTING_ADDITION_SOCKET_PATH_FORMAT.replacen("%s", &user_name, 1),
        ));
    true
}

pub(crate) fn is_scripting_addition_installed() -> bool {
    let osax_paths =
        SCRIPTING_ADDITION_BUNDLE_PATHS.get_or_init(build_scripting_addition_bundle_paths);

    let base_directory = CString::new(osax_paths.base_directory.as_str()).unwrap();
    let directory = unsafe { libc::opendir(base_directory.as_ptr()) };
    if directory.is_null() {
        return false;
    }

    unsafe { libc::closedir(directory) };
    true
}

pub(crate) fn is_scripting_addition_missing_or_outdated() -> i32 {
    autoreleasepool(|_pool| {
        let result: bool;

        if is_scripting_addition_installed() {
            let osax_paths =
                SCRIPTING_ADDITION_BUNDLE_PATHS.get_or_init(build_scripting_addition_bundle_paths);
            let payload_path = NSString::from_str(&osax_paths.payload_directory);
            let payload_bundle = NSBundle::bundleWithPath(&payload_path);
            let ns_version = payload_bundle.and_then(|payload_bundle| {
                payload_bundle.objectForInfoDictionaryKey(&NSString::from_str("CFBundleVersion"))
            });
            let ns_version_utf8_string: *const c_char = ns_version
                .as_deref()
                .map_or(core::ptr::null(), |ns_version| unsafe {
                    msg_send![ns_version, UTF8String]
                });

            let status = are_both_strings_present_and_equal(
                if ns_version_utf8_string.is_null() {
                    None
                } else {
                    unsafe { CStr::from_ptr(ns_version_utf8_string) }.to_str().ok()
                },
                Some(SCRIPTING_ADDITION_VERSION),
            );
            result = if status { false } else { true };
        } else {
            result = true;
        }

        result as i32
    })
}

pub(crate) fn remove_scripting_addition_bundle() -> bool {
    let osax_paths =
        SCRIPTING_ADDITION_BUNDLE_PATHS.get_or_init(build_scripting_addition_bundle_paths);

    let command = truncate_as_snprintf_would_into_a_fixed_string_buffer(format!(
        "{} {} {}",
        "rm -rf", osax_paths.base_directory, "2>/dev/null"
    ));

    let code = unsafe { libc::system(CString::new(command).unwrap().as_ptr()) };
    code == 0
}

pub(crate) fn install_scripting_addition_bundle_and_restart_dock() -> i32 {
    unsafe { libc::umask(libc::S_IWGRP | libc::S_IWOTH) };

    if is_scripting_addition_installed() && !remove_scripting_addition_bundle() {
        return 1;
    }

    'cleanup: {
        let osax_paths =
            SCRIPTING_ADDITION_BUNDLE_PATHS.get_or_init(build_scripting_addition_bundle_paths);

        if !create_scripting_addition_bundle_directories() {
            break 'cleanup;
        }

        let sa_plist =
            SCRIPTING_ADDITION_INFO_PLIST_TEMPLATE.replace("{}", SCRIPTING_ADDITION_VERSION);
        if !write_bytes_to_file_opened_with_mode(sa_plist.as_bytes(), &osax_paths.info_plist, "w") {
            break 'cleanup;
        }

        let sa_bundle_plist = SCRIPTING_ADDITION_PAYLOAD_INFO_PLIST_TEMPLATE
            .replace("{}", SCRIPTING_ADDITION_VERSION);
        if !write_bytes_to_file_opened_with_mode(
            sa_bundle_plist.as_bytes(),
            &osax_paths.payload_plist,
            "w",
        ) {
            break 'cleanup;
        }

        if !write_bytes_to_file_opened_with_mode(
            SCRIPTING_ADDITION_LOADER_BINARY,
            &osax_paths.binary_loader,
            "wb",
        ) {
            break 'cleanup;
        }

        if !write_bytes_to_file_opened_with_mode(
            SCRIPTING_ADDITION_PAYLOAD_BINARY,
            &osax_paths.binary_payload,
            "wb",
        ) {
            break 'cleanup;
        }

        make_scripting_addition_binaries_executable_and_ad_hoc_signed();
        terminate_dock_so_it_restarts();
        return 0;
    }

    remove_scripting_addition_bundle();
    2
}

pub(crate) fn validate_loaded_scripting_addition_updating_it_if_outdated() -> i32 {
    let mut attributes: u32 = 0;
    let mut version = String::new();
    let is_latest_version_installed = is_scripting_addition_missing_or_outdated() == 0;

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

    notify!("scripting-addition", "payload is outdated, restarting Dock.app..");
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
            boot_arguments.as_mut_ptr().cast::<c_void>(),
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
    let handle = unsafe {
        libc::popen(
            c"/Library/ScriptingAdditions/yabai.osax/Contents/MacOS/loader".as_ptr(),
            c"r".as_ptr(),
        )
    };
    if handle.is_null() {
        return false;
    }

    let result = unsafe { libc::pclose(handle) };
    if libc::WIFEXITED(result) {
        return libc::WEXITSTATUS(result) == 0;
    } else if libc::WIFSIGNALED(result) {
        return false;
    } else if libc::WIFSTOPPED(result) {
        return false;
    }

    false
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

        if is_scripting_addition_missing_or_outdated() != 0 {
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
            notify!("scripting-addition", "failed to inject payload into Dock.app!");
            result = 1;
            return result;
        }

        if store_scripting_addition_socket_path_of_sudo_user() {
            result = validate_loaded_scripting_addition_updating_it_if_outdated();
        }

        result
    })
}
