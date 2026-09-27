use crate::misc::helpers::{directory_exists, file_exists};
use crate::{error, warn};
use std::os::unix::fs::DirBuilderExt;

const _PATH_LAUNCHCTL: &str = "/bin/launchctl";
const _NAME_YABAI_PLIST: &str = "com.asmvik.yabai";
const _PATH_YABAI_PLIST: &str = "{0}/Library/LaunchAgents/com.asmvik.yabai.plist";

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

//
// NOTE(asmvik): A launchd service has the following states:
//
//          1. Installed / Uninstalled
//          2. Active (Enable / Disable)
//          3. Bootstrapped (Load / Unload)
//          4. Running (Start / Stop)
//

fn safe_exec(argv: &[&str], suppress_output: bool) -> i32 {
    let argument_strings: Vec<std::ffi::CString> = argv
        .iter()
        .map(|argument| std::ffi::CString::new(*argument).unwrap())
        .collect();
    let mut argument_pointers: Vec<*mut libc::c_char> = argument_strings
        .iter()
        .map(|argument| argument.as_ptr() as *mut libc::c_char)
        .collect();
    argument_pointers.push(core::ptr::null_mut());

    let mut process_id: libc::pid_t = 0;
    let mut actions: libc::posix_spawn_file_actions_t = unsafe { std::mem::zeroed() };
    unsafe { libc::posix_spawn_file_actions_init(&mut actions) };

    if suppress_output {
        let dev_null = std::ffi::CString::new("/dev/null").unwrap();
        unsafe {
            libc::posix_spawn_file_actions_addopen(
                &mut actions,
                libc::STDOUT_FILENO,
                dev_null.as_ptr(),
                libc::O_WRONLY | libc::O_APPEND,
                0,
            );
            libc::posix_spawn_file_actions_addopen(
                &mut actions,
                libc::STDERR_FILENO,
                dev_null.as_ptr(),
                libc::O_WRONLY | libc::O_APPEND,
                0,
            );
        }
    }

    let mut status: libc::c_int = unsafe {
        libc::posix_spawn(
            &mut process_id,
            argument_pointers[0],
            &actions,
            core::ptr::null(),
            argument_pointers.as_ptr(),
            core::ptr::null(),
        )
    };
    if status != 0 {
        return 1;
    }

    while unsafe { libc::waitpid(process_id, &mut status, 0) } == -1
        && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
    {
        unsafe { libc::usleep(1000) };
    }

    if libc::WIFSIGNALED(status) {
        1
    } else if libc::WIFSTOPPED(status) {
        1
    } else {
        libc::WEXITSTATUS(status)
    }
}

fn populate_plist_path() -> String {
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

fn service_install_internal(yabai_plist_path: &str) -> i32 {
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

pub fn service_start() -> i32 {
    let yabai_plist_path = populate_plist_path();
    if !file_exists(&yabai_plist_path) {
        warn!(
            "yabai: service file '{}' is not installed! attempting installation..\n",
            yabai_plist_path
        );

        let result = service_install_internal(&yabai_plist_path);
        if result != 0 {
            error!(
                "yabai: service file '{}' could not be installed! abort..\n",
                yabai_plist_path
            );
        }
    }

    let service_target = format!(
        "gui/{}/{}",
        unsafe { libc::getuid() } as i32,
        _NAME_YABAI_PLIST
    );

    let domain_target = format!("gui/{}", unsafe { libc::getuid() } as i32);

    //
    // NOTE(asmvik): Check if service is bootstrapped
    //

    let print_arguments = [_PATH_LAUNCHCTL, "print", &service_target];
    let is_bootstrapped = safe_exec(&print_arguments, true);

    if is_bootstrapped != 0 {
        //
        // NOTE(asmvik): Service is not bootstrapped and could be disabled.
        // There is no way to query if the service is disabled, and we cannot
        // bootstrap a disabled service. Try to enable the service. This will be
        // a no-op if the service is already enabled.
        //

        let enable_arguments = [_PATH_LAUNCHCTL, "enable", &service_target];
        safe_exec(&enable_arguments, false);

        //
        // NOTE(asmvik): Bootstrap service into the target domain.
        // This will also start the program **iff* RunAtLoad is set to true.
        //

        let bootstrap_arguments = [
            _PATH_LAUNCHCTL,
            "bootstrap",
            &domain_target,
            &yabai_plist_path,
        ];
        safe_exec(&bootstrap_arguments, false)
    } else {
        //
        // NOTE(asmvik): The service has already been bootstrapped.
        // Tell the bootstrapped service to launch immediately; it is an
        // error to bootstrap a service that has already been bootstrapped.
        //

        let kickstart_arguments = [_PATH_LAUNCHCTL, "kickstart", &service_target];
        safe_exec(&kickstart_arguments, false)
    }
}

pub fn service_restart() -> i32 {
    let yabai_plist_path = populate_plist_path();
    if !file_exists(&yabai_plist_path) {
        error!(
            "yabai: service file '{}' is not installed! abort..\n",
            yabai_plist_path
        );
    }

    let service_target = format!(
        "gui/{}/{}",
        unsafe { libc::getuid() } as i32,
        _NAME_YABAI_PLIST
    );

    let kickstart_arguments = [_PATH_LAUNCHCTL, "kickstart", "-k", &service_target];
    safe_exec(&kickstart_arguments, false)
}

pub fn service_stop() -> i32 {
    let yabai_plist_path = populate_plist_path();
    if !file_exists(&yabai_plist_path) {
        error!(
            "yabai: service file '{}' is not installed! abort..\n",
            yabai_plist_path
        );
    }

    let service_target = format!(
        "gui/{}/{}",
        unsafe { libc::getuid() } as i32,
        _NAME_YABAI_PLIST
    );

    let domain_target = format!("gui/{}", unsafe { libc::getuid() } as i32);

    //
    // NOTE(asmvik): Check if service is bootstrapped
    //

    let print_arguments = [_PATH_LAUNCHCTL, "print", &service_target];
    let is_bootstrapped = safe_exec(&print_arguments, true);

    if is_bootstrapped != 0 {
        //
        // NOTE(asmvik): Service is not bootstrapped, but the program
        // could still be running an instance that was started **while the service
        // was bootstrapped**, so we tell it to stop said service.
        //

        let kill_arguments = [_PATH_LAUNCHCTL, "kill", "SIGTERM", &service_target];
        safe_exec(&kill_arguments, false)
    } else {
        //
        // NOTE(asmvik): Service is bootstrapped; we stop a potentially
        // running instance of the program and unload the service, making it
        // not trigger automatically in the future.
        //
        // This is NOT the same as disabling the service, which will prevent
        // it from being boostrapped in the future (without explicitly re-enabling
        // it first).
        //

        let bootout_arguments = [
            _PATH_LAUNCHCTL,
            "bootout",
            &domain_target,
            &yabai_plist_path,
        ];
        safe_exec(&bootout_arguments, false);

        let disable_arguments = [_PATH_LAUNCHCTL, "disable", &service_target];
        safe_exec(&disable_arguments, false)
    }
}
