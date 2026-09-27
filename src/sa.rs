use core::ffi::{CStr, c_char, c_void};
use std::ffi::CString;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;

use objc2::rc::autoreleasepool;
use objc2::{msg_send, sel};

use crate::ffi::appkit::NSRunningApplication;
use crate::ffi::foundation::{NSBundle, NSString};
use crate::ffi::libsystem::{
    CSR_ALLOW_TASK_FOR_PID, CSR_ALLOW_UNRESTRICTED_FS, csr_get_active_config,
};
use crate::ffi::skylight::SLSWindowIsOrderedIn;
use crate::globals::{CONNECTION, SA_SOCKET_FILE};
use crate::handles::{SpaceId, WindowId};
use crate::misc::helpers::{is_root, socket_close, socket_connect, socket_open, string_equals};
use crate::misc::macros::MAXLEN;
use crate::view::WindowAnimation;
use crate::{notify, warn};

include!(concat!(env!("OUT_DIR"), "/osax_common.rs"));

pub(crate) static OSAX_PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload"));
pub(crate) static OSAX_LOADER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/loader"));

pub(crate) struct OsaxPaths {
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

static OSAX_PATHS: OnceLock<OsaxPaths> = OnceLock::new();

pub(crate) const SA_PLIST: &str = concat!(
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

pub(crate) const SA_BUNDLE_PLIST: &str = concat!(
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

fn truncate_as_snprintf_would_into_a_maxlen_buffer(mut text: String) -> String {
    let mut length = text.len().min(MAXLEN - 1);
    while !text.is_char_boundary(length) {
        length -= 1;
    }
    text.truncate(length);
    text
}

pub(crate) fn scripting_addition_set_path() -> OsaxPaths {
    let base_directory = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{}",
        "/Library/ScriptingAdditions/yabai.osax"
    ));

    let contents_directory = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{}/{}",
        base_directory, "Contents"
    ));
    let contents_macos_directory = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{}/{}",
        contents_directory, "MacOS"
    ));
    let contents_resources_directory = truncate_as_snprintf_would_into_a_maxlen_buffer(
        format!("{}/{}", contents_directory, "Resources"),
    );
    let info_plist = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{}/{}",
        contents_directory, "Info.plist"
    ));

    let payload_directory = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{}/{}",
        contents_resources_directory, "payload.bundle"
    ));
    let payload_contents_directory = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{}/{}",
        payload_directory, "Contents"
    ));
    let payload_contents_macos_directory = truncate_as_snprintf_would_into_a_maxlen_buffer(
        format!("{}/{}", payload_contents_directory, "MacOS"),
    );
    let payload_plist = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{}/{}",
        payload_contents_directory, "Info.plist"
    ));

    let binary_loader = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{}/{}",
        contents_macos_directory, "loader"
    ));
    let binary_payload = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{}/{}",
        payload_contents_macos_directory, "payload"
    ));

    OsaxPaths {
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

pub(crate) fn scripting_addition_create_directory() -> bool {
    let osax_paths = OSAX_PATHS.get_or_init(scripting_addition_set_path);
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

pub(crate) fn scripting_addition_write_file(buffer: &[u8], file: &str, file_mode: &str) -> bool {
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

pub(crate) fn scripting_addition_prepare_binaries() {
    let osax_paths = OSAX_PATHS.get_or_init(scripting_addition_set_path);

    let command = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{} {}",
        "chmod +x", osax_paths.binary_loader
    ));
    unsafe { libc::system(CString::new(command).unwrap().as_ptr()) };

    let command = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{} {} {}",
        "codesign -f -s -", osax_paths.binary_loader, "2>/dev/null"
    ));
    unsafe { libc::system(CString::new(command).unwrap().as_ptr()) };

    let command = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{} {}",
        "chmod +x", osax_paths.binary_payload
    ));
    unsafe { libc::system(CString::new(command).unwrap().as_ptr()) };

    let command = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{} {} {}",
        "codesign -f -s -", osax_paths.binary_payload, "2>/dev/null"
    ));
    unsafe { libc::system(CString::new(command).unwrap().as_ptr()) };
}

pub(crate) fn scripting_addition_restart_dock() {
    let dock =
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
            "com.apple.dock",
        ));
    unsafe { dock.makeObjectsPerformSelector(sel!(terminate)) };
}

pub(crate) fn scripting_addition_set_socket_path() -> bool {
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
    let _ = SA_SOCKET_FILE.set(truncate_as_snprintf_would_into_a_maxlen_buffer(
        SA_SOCKET_PATH_FMT.replacen("%s", &user_name, 1),
    ));
    true
}

pub(crate) fn scripting_addition_is_installed() -> bool {
    let osax_paths = OSAX_PATHS.get_or_init(scripting_addition_set_path);

    let base_directory = CString::new(osax_paths.base_directory.as_str()).unwrap();
    let directory = unsafe { libc::opendir(base_directory.as_ptr()) };
    if directory.is_null() {
        return false;
    }

    unsafe { libc::closedir(directory) };
    true
}

pub(crate) fn scripting_addition_check() -> i32 {
    autoreleasepool(|_pool| {
        let result: bool;

        if scripting_addition_is_installed() {
            let osax_paths = OSAX_PATHS.get_or_init(scripting_addition_set_path);
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

            let status = string_equals(
                if ns_version_utf8_string.is_null() {
                    None
                } else {
                    unsafe { CStr::from_ptr(ns_version_utf8_string) }.to_str().ok()
                },
                Some(OSAX_VERSION),
            );
            result = if status { false } else { true };
        } else {
            result = true;
        }

        result as i32
    })
}

pub(crate) fn scripting_addition_remove() -> bool {
    let osax_paths = OSAX_PATHS.get_or_init(scripting_addition_set_path);

    let command = truncate_as_snprintf_would_into_a_maxlen_buffer(format!(
        "{} {} {}",
        "rm -rf", osax_paths.base_directory, "2>/dev/null"
    ));

    let code = unsafe { libc::system(CString::new(command).unwrap().as_ptr()) };
    code == 0
}

pub(crate) fn scripting_addition_install() -> i32 {
    unsafe { libc::umask(libc::S_IWGRP | libc::S_IWOTH) };

    if scripting_addition_is_installed() && !scripting_addition_remove() {
        return 1;
    }

    'cleanup: {
        let osax_paths = OSAX_PATHS.get_or_init(scripting_addition_set_path);

        if !scripting_addition_create_directory() {
            break 'cleanup;
        }

        let sa_plist = SA_PLIST.replace("{}", OSAX_VERSION);
        if !scripting_addition_write_file(sa_plist.as_bytes(), &osax_paths.info_plist, "w") {
            break 'cleanup;
        }

        let sa_bundle_plist = SA_BUNDLE_PLIST.replace("{}", OSAX_VERSION);
        if !scripting_addition_write_file(
            sa_bundle_plist.as_bytes(),
            &osax_paths.payload_plist,
            "w",
        ) {
            break 'cleanup;
        }

        if !scripting_addition_write_file(OSAX_LOADER, &osax_paths.binary_loader, "wb") {
            break 'cleanup;
        }

        if !scripting_addition_write_file(OSAX_PAYLOAD, &osax_paths.binary_payload, "wb") {
            break 'cleanup;
        }

        scripting_addition_prepare_binaries();
        scripting_addition_restart_dock();
        return 0;
    }

    scripting_addition_remove();
    2
}

pub(crate) fn scripting_addition_request_handshake(
    version: &mut String,
    attributes: &mut u32,
) -> bool {
    let mut socket_file_descriptor: i32 = 0;
    let mut result = false;
    let mut response = [0u8; libc::BUFSIZ as usize];
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    bytes[0] = 0x01;
    bytes[1] = 0x00;
    bytes[2] = SaOpcode::Handshake as u8;

    if socket_open(&mut socket_file_descriptor) {
        'out: {
            if socket_connect(
                socket_file_descriptor,
                SA_SOCKET_FILE.get().map_or("", String::as_str),
            ) {
                if unsafe {
                    libc::send(
                        socket_file_descriptor,
                        bytes.as_ptr().cast::<c_void>(),
                        3,
                        0,
                    )
                } != -1
                {
                    let length = unsafe {
                        libc::recv(
                            socket_file_descriptor,
                            response.as_mut_ptr().cast::<c_void>(),
                            response.len() - 1,
                            0,
                        )
                    } as i32;
                    if length <= 0 {
                        break 'out;
                    }

                    let mut zero = 0;
                    while response[zero] != b'\0' {
                        zero += 1;
                    }

                    debug_assert!(response[zero] == b'\0');
                    let Some(attribute_bytes) = response.get(zero + 1..zero + 1 + size_of::<u32>())
                    else {
                        break 'out;
                    };
                    *version = String::from_utf8_lossy(&response[..zero]).into_owned();
                    *attributes = u32::from_ne_bytes(attribute_bytes.try_into().unwrap());

                    result = true;
                }
            }
        }

        socket_close(socket_file_descriptor);
    }

    result
}

pub(crate) fn scripting_addition_perform_validation() -> i32 {
    let mut attributes: u32 = 0;
    let mut version = String::new();
    let is_latest_version_installed = scripting_addition_check() == 0;

    if !scripting_addition_request_handshake(&mut version, &mut attributes) {
        notify!("scripting-addition", "connection failed!");
        return 1;
    }

    if string_equals(Some(&version), Some(OSAX_VERSION)) {
        if (attributes & OSAX_ATTRIB_ALL) == OSAX_ATTRIB_ALL {
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
        return scripting_addition_install();
    }

    notify!("scripting-addition", "payload is outdated, restarting Dock.app..");
    scripting_addition_restart_dock();
    0
}

pub(crate) fn scripting_addition_is_sip_friendly() -> bool {
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
pub(crate) fn scripting_addition_is_arm64e_enabled() -> bool {
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

pub(crate) fn mach_loader_inject_payload() -> bool {
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

pub(crate) fn scripting_addition_uninstall() -> i32 {
    if !scripting_addition_is_sip_friendly() {
        warn!(
            "yabai: System Integrity Protection: Filesystem Protections and Debugging Restrictions must be disabled!\n"
        );
        notify!(
            "scripting-addition",
            "System Integrity Protection: Filesystem Protections and Debugging Restrictions must be disabled!"
        );
        return 1;
    }

    if !is_root() {
        warn!("yabai: scripting-addition must be uninstalled as root!\n");
        notify!("scripting-addition", "must be uninstalled as root!");
        return 1;
    }

    if !scripting_addition_is_installed() {
        return 0;
    }
    if !scripting_addition_remove() {
        return -1;
    }
    0
}

pub(crate) fn scripting_addition_load() -> i32 {
    autoreleasepool(|_pool| {
        let mut result = 0;

        if !is_root() {
            warn!("yabai: scripting-addition must be loaded as root!\n");
            notify!("scripting-addition", "must be loaded as root!");
            result = 1;
            return result;
        }

        if !scripting_addition_is_sip_friendly() {
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

        if scripting_addition_check() != 0 {
            result = scripting_addition_install();
            return result;
        }

        #[cfg(target_arch = "aarch64")]
        {
            if !scripting_addition_is_arm64e_enabled() {
                warn!("yabai: missing required nvram boot-arg '-arm64e_preview_abi'!\n");
                notify!(
                    "scripting-addition",
                    "missing required nvram boot-arg '-arm64e_preview_abi'!"
                );
                result = 1;
                return result;
            }
        }

        if !mach_loader_inject_payload() {
            warn!("yabai: scripting-addition failed to inject payload into Dock.app!\n");
            notify!("scripting-addition", "failed to inject payload into Dock.app!");
            result = 1;
            return result;
        }

        if scripting_addition_set_socket_path() {
            result = scripting_addition_perform_validation();
        }

        result
    })
}

pub(crate) fn pack(bytes: &mut [u8], length: &mut i16, value: &[u8]) -> bool {
    let offset = *length as usize;
    let Some(destination) = bytes.get_mut(offset..offset + value.len()) else {
        return false;
    };
    destination.copy_from_slice(value);
    *length += value.len() as i16;
    true
}

pub(crate) fn sa_payload_send(bytes: &mut [u8], length: i16, opcode: SaOpcode) -> bool {
    bytes[..size_of::<i16>()]
        .copy_from_slice(&((length as usize - size_of::<i16>()) as i16).to_ne_bytes());
    bytes[size_of::<i16>()] = opcode as u8;
    scripting_addition_send_bytes(&bytes[..length as usize])
}

pub(crate) fn scripting_addition_send_bytes(bytes: &[u8]) -> bool {
    let mut socket_file_descriptor: i32 = 0;
    let mut dummy: c_char = 0;
    let mut result = false;

    if socket_open(&mut socket_file_descriptor) {
        if socket_connect(
            socket_file_descriptor,
            SA_SOCKET_FILE.get().map_or("", String::as_str),
        ) {
            if unsafe {
                libc::send(
                    socket_file_descriptor,
                    bytes.as_ptr().cast::<c_void>(),
                    bytes.len(),
                    0,
                )
            } != -1
            {
                unsafe {
                    libc::recv(
                        socket_file_descriptor,
                        (&raw mut dummy).cast::<c_void>(),
                        1,
                        0,
                    )
                };
                result = true;
            }
        }

        socket_close(socket_file_descriptor);
    }

    result
}

pub(crate) fn scripting_addition_focus_space(space_id: SpaceId) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &space_id.0.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::SpaceFocus)
}

pub(crate) fn scripting_addition_create_space(space_id: SpaceId) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &space_id.0.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::SpaceCreate)
}

pub(crate) fn scripting_addition_destroy_space(space_id: SpaceId) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &space_id.0.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::SpaceDestroy)
}

pub(crate) fn scripting_addition_move_space_to_display(
    source_space_id: SpaceId,
    destination_space_id: SpaceId,
    source_previous_space_id: SpaceId,
    focus: bool,
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &source_space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &destination_space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &source_previous_space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &[focus as u8]) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::SpaceMove)
}

pub(crate) fn scripting_addition_move_space_after_space(
    source_space_id: SpaceId,
    destination_space_id: SpaceId,
    focus: bool,
) -> bool {
    let dummy_space_id: u64 = 0;
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &source_space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &destination_space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &dummy_space_id.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &[focus as u8]) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::SpaceMove)
}

pub(crate) fn scripting_addition_move_window(window_id: WindowId, x: i32, y: i32) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &x.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &y.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowMove)
}

pub(crate) fn scripting_addition_set_opacity(
    window_id: WindowId,
    opacity: f32,
    duration: f32,
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &opacity.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &duration.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(
        &mut bytes,
        length,
        if duration > 0.0f32 {
            SaOpcode::WindowOpacityFade
        } else {
            SaOpcode::WindowOpacity
        },
    )
}

pub(crate) fn scripting_addition_set_layer(window_id: WindowId, layer: i32) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &layer.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowLayer)
}

pub(crate) fn scripting_addition_set_sticky(window_id: WindowId, sticky: bool) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &[sticky as u8]) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowSticky)
}

pub(crate) fn scripting_addition_set_shadow(window_id: WindowId, shadow: bool) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &[shadow as u8]) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowShadow)
}

pub(crate) fn scripting_addition_scale_window(
    window_id: WindowId,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &x.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &y.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &width.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &height.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowScale)
}

pub(crate) fn scripting_addition_swap_window_proxy_in(animation_list: &[WindowAnimation]) -> bool {
    let dummy_window_id: u32 = 0;
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &(animation_list.len() as i32).to_ne_bytes()) {
        return false;
    }
    for animation in animation_list {
        if animation.skip.load(Ordering::Relaxed) {
            if !pack(&mut bytes, &mut length, &dummy_window_id.to_ne_bytes()) {
                return false;
            }
        } else {
            if !pack(&mut bytes, &mut length, &animation.window_id.0.to_ne_bytes()) {
                return false;
            }
            if !pack(
                &mut bytes,
                &mut length,
                &animation.proxy.id.load(Ordering::Relaxed).to_ne_bytes(),
            ) {
                return false;
            }
        }
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowSwapProxyIn)
}

pub(crate) fn scripting_addition_swap_window_proxy_out(animation_list: &[WindowAnimation]) -> bool {
    let dummy_window_id: u32 = 0;
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &(animation_list.len() as i32).to_ne_bytes()) {
        return false;
    }
    for animation in animation_list {
        if animation.skip.load(Ordering::Relaxed) {
            if !pack(&mut bytes, &mut length, &dummy_window_id.to_ne_bytes()) {
                return false;
            }
        } else {
            if !pack(&mut bytes, &mut length, &animation.window_id.0.to_ne_bytes()) {
                return false;
            }
            if !pack(
                &mut bytes,
                &mut length,
                &animation.proxy.id.load(Ordering::Relaxed).to_ne_bytes(),
            ) {
                return false;
            }
        }
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowSwapProxyOut)
}

pub(crate) fn scripting_addition_order_window(
    a_window_id: WindowId,
    order: i32,
    b_window_id: WindowId,
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &a_window_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &order.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &b_window_id.0.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowOrder)
}

pub(crate) fn scripting_addition_order_window_in(window_list: &[WindowId]) -> bool {
    let dummy_window_id: u32 = 0;
    let mut ordered_in: u8 = 0;

    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &(window_list.len() as i32).to_ne_bytes()) {
        return false;
    }
    for window_id in window_list {
        unsafe { SLSWindowIsOrderedIn(*CONNECTION.get().unwrap(), window_id.0, &mut ordered_in) };
        if ordered_in != 0 {
            if !pack(&mut bytes, &mut length, &dummy_window_id.to_ne_bytes()) {
                return false;
            }
        } else {
            if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
                return false;
            }
        }
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowOrderIn)
}

pub(crate) fn scripting_addition_move_window_list_to_space(
    space_id: SpaceId,
    window_list: &[WindowId],
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &(window_list.len() as i32).to_ne_bytes()) {
        return false;
    }
    for window_id in window_list {
        if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
            return false;
        }
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowListToSpace)
}

pub(crate) fn scripting_addition_move_window_to_space(
    space_id: SpaceId,
    window_id: WindowId,
) -> bool {
    let mut bytes = [0u8; SA_SOCKET_BUFF_LEN];
    let mut length: i16 = 1 + 2;
    if !pack(&mut bytes, &mut length, &space_id.0.to_ne_bytes()) {
        return false;
    }
    if !pack(&mut bytes, &mut length, &window_id.0.to_ne_bytes()) {
        return false;
    }
    sa_payload_send(&mut bytes, length, SaOpcode::WindowToSpace)
}
