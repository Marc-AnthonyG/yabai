use crate::error;
use crate::support::filesystem::{is_existing_directory, is_existing_file_that_is_not_a_directory};
use std::os::unix::fs::DirBuilderExt;

macro_rules! launchd_service_plist_template {
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

pub(crate) fn build_launchd_service_plist_path() -> String {
    let home_reference = crate::ffi::foundation::NSHomeDirectoryForUser(None);
    let home = match home_reference {
        Some(home_reference) => home_reference.to_string(),
        None => error!("yabai: unable to retrieve home directory! abort..\n"),
    };

    format!("{}/Library/LaunchAgents/com.asmvik.yabai.plist", home)
}

fn build_launchd_service_plist_contents() -> String {
    let Some(user) = std::env::var_os("USER") else {
        error!("yabai: 'env USER' not set! abort..\n");
    };
    let user = user.to_string_lossy();

    let Some(path_env) = std::env::var_os("PATH") else {
        error!("yabai: 'env PATH' not set! abort..\n");
    };
    let path_env = path_env.to_string_lossy();

    let Ok(executable_path) = std::env::current_exe() else {
        error!("yabai: unable to retrieve path of executable! abort..\n");
    };
    let executable_path = executable_path.to_string_lossy();

    format!(
        launchd_service_plist_template!(),
        executable_path, path_env, user
    )
}

fn create_parent_directory_of_plist_if_missing(yabai_plist_path: &str) {
    let parent = std::path::Path::new(yabai_plist_path).parent().unwrap();

    if !is_existing_directory(&parent.to_string_lossy()) {
        let _ = std::fs::DirBuilder::new().mode(0o755).create(parent);
    }
}

pub(crate) fn write_launchd_service_plist(yabai_plist_path: &str) -> i32 {
    let yabai_plist = build_launchd_service_plist_contents();
    create_parent_directory_of_plist_if_missing(yabai_plist_path);

    let Ok(mut handle) = std::fs::File::create(yabai_plist_path) else {
        return 1;
    };

    match std::io::Write::write_all(&mut handle, yabai_plist.as_bytes()) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

pub fn install_launchd_service() -> i32 {
    let yabai_plist_path = build_launchd_service_plist_path();

    if is_existing_file_that_is_not_a_directory(&yabai_plist_path) {
        error!(
            "yabai: service file '{}' is already installed! abort..\n",
            yabai_plist_path
        );
    }

    write_launchd_service_plist(&yabai_plist_path)
}

pub fn uninstall_launchd_service() -> i32 {
    let yabai_plist_path = build_launchd_service_plist_path();

    if !is_existing_file_that_is_not_a_directory(&yabai_plist_path) {
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
