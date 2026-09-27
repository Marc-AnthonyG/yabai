use crate::error;
use crate::support::filesystem::{directory_exists, file_exists};
use std::os::unix::fs::DirBuilderExt;

macro_rules! yabai_plist_template {
    () => {
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
            "<plist version=\"1.0\">\n",
            "<dict>\n",
            "    <key>Label</key>\n",
            "    <string>com.asmvik.yabai</string>\n",
            "    <key>ProgramArguments</key>\n",
            "    <array>\n",
            "        <string>{0}</string>\n",
            "    </array>\n",
            "    <key>EnvironmentVariables</key>\n",
            "    <dict>\n",
            "        <key>PATH</key>\n",
            "        <string>{1}</string>\n",
            "    </dict>\n",
            "    <key>RunAtLoad</key>\n",
            "    <true/>\n",
            "    <key>KeepAlive</key>\n",
            "    <dict>\n",
            "        <key>SuccessfulExit</key>\n",
            " \t     <false/>\n",
            " \t     <key>Crashed</key>\n",
            " \t     <true/>\n",
            "    </dict>\n",
            "    <key>StandardOutPath</key>\n",
            "    <string>/tmp/yabai_{2}.out.log</string>\n",
            "    <key>StandardErrorPath</key>\n",
            "    <string>/tmp/yabai_{2}.err.log</string>\n",
            "    <key>ProcessType</key>\n",
            "    <string>Interactive</string>\n",
            "    <key>Nice</key>\n",
            "    <integer>-20</integer>\n",
            "</dict>\n",
            "</plist>",
        )
    };
}

pub(crate) fn populate_plist_path() -> String {
    let home_reference = crate::ffi::foundation::NSHomeDirectoryForUser(None);
    let home = match home_reference {
        Some(home_reference) => home_reference.to_string(),
        None => error!("yabai: unable to retrieve home directory! abort..\n"),
    };

    format!("{}/Library/LaunchAgents/com.asmvik.yabai.plist", home)
}

fn populate_plist() -> String {
    let user = match std::env::var("USER") {
        Ok(user) => user,
        Err(_) => error!("yabai: 'env USER' not set! abort..\n"),
    };

    let path_env = match std::env::var("PATH") {
        Ok(path_env) => path_env,
        Err(_) => error!("yabai: 'env PATH' not set! abort..\n"),
    };

    let mut executable_path_buffer = [0u8; 4096];
    let mut executable_path_size: u32 = executable_path_buffer.len() as u32;
    if unsafe {
        crate::ffi::libsystem::_NSGetExecutablePath(
            executable_path_buffer.as_mut_ptr() as *mut libc::c_char,
            &mut executable_path_size,
        )
    } < 0
    {
        error!("yabai: unable to retrieve path of executable! abort..\n");
    }
    let nul_position = executable_path_buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(executable_path_buffer.len());
    let executable_path =
        String::from_utf8_lossy(&executable_path_buffer[..nul_position]).into_owned();

    format!(yabai_plist_template!(), executable_path, path_env, user)
}

fn ensure_directory_exists(yabai_plist_path: &str) {
    let parent = std::path::Path::new(yabai_plist_path).parent().unwrap();

    if !directory_exists(&parent.to_string_lossy()) {
        let _ = std::fs::DirBuilder::new().mode(0o755).create(parent);
    }
}

pub(crate) fn service_install_internal(yabai_plist_path: &str) -> i32 {
    let yabai_plist = populate_plist();
    ensure_directory_exists(yabai_plist_path);

    let Ok(mut handle) = std::fs::File::create(yabai_plist_path) else {
        return 1;
    };

    match std::io::Write::write_all(&mut handle, yabai_plist.as_bytes()) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

pub fn service_install() -> i32 {
    let yabai_plist_path = populate_plist_path();

    if file_exists(&yabai_plist_path) {
        error!(
            "yabai: service file '{}' is already installed! abort..\n",
            yabai_plist_path
        );
    }

    service_install_internal(&yabai_plist_path)
}

pub fn service_uninstall() -> i32 {
    let yabai_plist_path = populate_plist_path();

    if !file_exists(&yabai_plist_path) {
        error!(
            "yabai: service file '{}' is not installed! abort..\n",
            yabai_plist_path
        );
    }

    if std::fs::remove_file(&yabai_plist_path).is_ok() {
        0
    } else {
        1
    }
}
