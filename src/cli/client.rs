use std::io::Write;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

use crate::command::DaemonCommand;
use crate::protocol::reply::{DaemonReply, read_reply_until_the_daemon_closes_the_stream};
use crate::protocol::request::{request_from_this_client, write_request_after_its_length};
use crate::protocol::socket_path::message_socket_path_of_user;

const EXIT_STATUS_WHEN_THE_WINDOW_MANAGER_REPORTS_A_FAILURE: i32 = 1;
const EXIT_STATUS_WHEN_NO_WINDOW_MANAGER_ANSWERS: i32 = 3;

pub(crate) fn send_command_to_the_running_window_manager_and_print_its_reply(
    command: DaemonCommand,
) -> i32 {
    match exchange_the_command_for_the_reply_of_the_running_window_manager(command) {
        Ok(reply) => print_reply_returning_the_exit_status_it_calls_for(&reply),
        Err(failure) => {
            eprintln!("yabai: {failure}");
            EXIT_STATUS_WHEN_NO_WINDOW_MANAGER_ANSWERS
        }
    }
}

fn exchange_the_command_for_the_reply_of_the_running_window_manager(
    command: DaemonCommand,
) -> Result<DaemonReply, String> {
    let user = std::env::var("USER").map_err(|_| String::from("'env USER' is not set"))?;
    let socket_path = message_socket_path_of_user(&user);
    let stream = UnixStream::connect(&socket_path)
        .map_err(|error| format!("could not reach a running yabai at '{socket_path}': {error}"))?;

    write_request_after_its_length(&stream, &request_from_this_client(command))
        .map_err(|error| format!("could not send the command: {error}"))?;
    let _ = stream.shutdown(Shutdown::Write);

    read_reply_until_the_daemon_closes_the_stream(&stream).map_err(|error| {
        format!("{error}; if the running yabai is an older build, run `yabai service restart`")
    })
}

fn print_reply_returning_the_exit_status_it_calls_for(reply: &DaemonReply) -> i32 {
    let mut standard_output = std::io::stdout().lock();
    let _ = standard_output.write_all(reply.standard_output.as_bytes());
    let _ = standard_output.flush();

    let mut standard_error = std::io::stderr().lock();
    for failure in &reply.failures {
        let _ = writeln!(standard_error, "{failure}");
    }

    if reply.failures.is_empty() {
        libc::EXIT_SUCCESS
    } else {
        EXIT_STATUS_WHEN_THE_WINDOW_MANAGER_REPORTS_A_FAILURE
    }
}
