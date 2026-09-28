use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use crate::command::DaemonCommand;

const VERSION_OF_THIS_BINARY: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize)]
pub(crate) struct DaemonRequest {
    pub(crate) client_version: String,
    pub(crate) command: DaemonCommand,
}

pub(crate) fn request_from_this_client(command: DaemonCommand) -> DaemonRequest {
    DaemonRequest {
        client_version: VERSION_OF_THIS_BINARY.to_owned(),
        command,
    }
}

pub(crate) fn write_request_after_its_length(
    mut writer: impl Write,
    request: &DaemonRequest,
) -> std::io::Result<()> {
    let request_as_json = serde_json::to_vec(request)?;
    let length_of_the_request = u32::try_from(request_as_json.len())
        .map_err(|_| std::io::Error::other("the request does not fit a 32-bit length"))?;
    writer.write_all(&length_of_the_request.to_ne_bytes())?;
    writer.write_all(&request_as_json)
}

pub(crate) fn read_request_after_its_length(
    mut reader: impl Read,
) -> Result<DaemonRequest, String> {
    let mut length_bytes = [0u8; size_of::<u32>()];
    reader
        .read_exact(&mut length_bytes)
        .map_err(|error| format!("could not read the length of the request: {error}"))?;
    let length_of_the_request = u32::from_ne_bytes(length_bytes);

    let mut request_as_json = Vec::new();
    reader
        .take(u64::from(length_of_the_request))
        .read_to_end(&mut request_as_json)
        .map_err(|error| format!("could not read the request: {error}"))?;
    if request_as_json.len() != length_of_the_request as usize {
        return Err(format!(
            "the request ended after {} of its {length_of_the_request} bytes",
            request_as_json.len()
        ));
    }

    serde_json::from_slice(&request_as_json)
        .map_err(|error| format!("could not understand the request: {error}"))
}

pub(crate) fn command_of_a_request_from_a_client_of_the_same_version(
    request: DaemonRequest,
) -> Result<DaemonCommand, String> {
    if request.client_version != VERSION_OF_THIS_BINARY {
        return Err(format!(
            "yabai {} cannot drive the running yabai {VERSION_OF_THIS_BINARY}; run `yabai service restart`",
            request.client_version
        ));
    }
    Ok(request.command)
}

#[cfg(test)]
mod tests {
    use super::{
        DaemonRequest, command_of_a_request_from_a_client_of_the_same_version,
        read_request_after_its_length, request_from_this_client, write_request_after_its_length,
    };
    use crate::command::DaemonCommand;

    fn command_not_yet_typed() -> DaemonCommand {
        DaemonCommand::NotYetTyped {
            arguments: vec![String::from("window"), String::from("--focus")],
        }
    }

    #[test]
    fn a_request_read_back_after_its_length_is_the_request_written() {
        let mut framed_request = Vec::new();
        write_request_after_its_length(
            &mut framed_request,
            &request_from_this_client(command_not_yet_typed()),
        )
        .unwrap();

        let request_read_back = read_request_after_its_length(framed_request.as_slice()).unwrap();

        assert_eq!(
            serde_json::to_string(&request_read_back.command).unwrap(),
            serde_json::to_string(&command_not_yet_typed()).unwrap()
        );
        assert_eq!(request_read_back.client_version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn the_length_before_the_request_is_native_endian_and_counts_the_json_bytes() {
        let mut framed_request = Vec::new();
        write_request_after_its_length(
            &mut framed_request,
            &request_from_this_client(command_not_yet_typed()),
        )
        .unwrap();

        let (length_bytes, request_as_json) = framed_request.split_at(size_of::<u32>());

        assert_eq!(
            u32::from_ne_bytes(length_bytes.try_into().unwrap()) as usize,
            request_as_json.len()
        );
        assert!(serde_json::from_slice::<serde_json::Value>(request_as_json).is_ok());
    }

    #[test]
    fn a_request_cut_short_is_refused() {
        let mut framed_request = Vec::new();
        write_request_after_its_length(
            &mut framed_request,
            &request_from_this_client(command_not_yet_typed()),
        )
        .unwrap();
        framed_request.truncate(framed_request.len() - 1);

        assert!(read_request_after_its_length(framed_request.as_slice()).is_err());
    }

    #[test]
    fn a_request_from_a_client_of_another_version_is_refused_naming_both_versions() {
        let request = DaemonRequest {
            client_version: String::from("0.0.1"),
            command: command_not_yet_typed(),
        };

        let failure = command_of_a_request_from_a_client_of_the_same_version(request)
            .err()
            .unwrap();

        assert!(failure.contains("0.0.1"));
        assert!(failure.contains(env!("CARGO_PKG_VERSION")));
    }
}
