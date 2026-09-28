use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub(crate) struct DaemonReply {
    pub(crate) standard_output: String,
    pub(crate) failures: Vec<String>,
}

impl DaemonReply {
    pub(crate) fn failing_with(failure: String) -> DaemonReply {
        DaemonReply {
            standard_output: String::new(),
            failures: vec![failure],
        }
    }

    pub(crate) fn printing_or_failing_with_every_failure(
        outcome: Result<String, Vec<String>>,
    ) -> DaemonReply {
        match outcome {
            Ok(standard_output) => DaemonReply {
                standard_output,
                failures: Vec::new(),
            },
            Err(failures) => DaemonReply {
                standard_output: String::new(),
                failures,
            },
        }
    }
}

pub(crate) fn write_reply(writer: impl Write, reply: &DaemonReply) -> std::io::Result<()> {
    serde_json::to_writer(writer, reply).map_err(std::io::Error::from)
}

pub(crate) fn read_reply_until_the_daemon_closes_the_stream(
    mut reader: impl Read,
) -> Result<DaemonReply, String> {
    let mut reply_as_json = Vec::new();
    reader
        .read_to_end(&mut reply_as_json)
        .map_err(|error| format!("could not read the reply: {error}"))?;
    serde_json::from_slice(&reply_as_json)
        .map_err(|error| format!("could not understand the reply: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{DaemonReply, read_reply_until_the_daemon_closes_the_stream, write_reply};

    #[test]
    fn a_reply_read_back_is_the_reply_written() {
        let reply = DaemonReply {
            standard_output: String::from("[]\n"),
            failures: vec![String::from("could not locate the selected window.")],
        };
        let mut reply_as_json = Vec::new();
        write_reply(&mut reply_as_json, &reply).unwrap();

        assert_eq!(
            read_reply_until_the_daemon_closes_the_stream(reply_as_json.as_slice()).unwrap(),
            reply
        );
    }

    #[test]
    fn the_failure_byte_an_older_daemon_answers_with_is_not_a_reply() {
        let answer_of_an_older_daemon = b"\x07unknown domain '{\"client_version\"'\n";

        assert!(
            read_reply_until_the_daemon_closes_the_stream(answer_of_an_older_daemon.as_slice())
                .is_err()
        );
    }
}
