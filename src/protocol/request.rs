use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use crate::command::DaemonCommand;

const VERSION_OF_THIS_BINARY: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize)]
pub(crate) struct DaemonRequest {
    pub(crate) client_version: String,
    pub(crate) command: DaemonCommand,
}

#[derive(Deserialize)]
struct DaemonRequestWhoseCommandIsNotReadYet {
    client_version: String,
    command: serde_json::Value,
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

pub(crate) fn read_the_json_of_a_request_after_its_length(
    mut reader: impl Read,
) -> Result<Vec<u8>, String> {
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
    Ok(request_as_json)
}

pub(crate) fn command_of_a_request_from_a_client_of_the_same_version(
    request_as_json: &[u8],
) -> Result<DaemonCommand, String> {
    let request: DaemonRequestWhoseCommandIsNotReadYet = serde_json::from_slice(request_as_json)
        .map_err(|error| format!("could not understand the request: {error}"))?;
    if request.client_version != VERSION_OF_THIS_BINARY {
        return Err(format!(
            "yabai {} cannot drive the running yabai {VERSION_OF_THIS_BINARY}; run `yabai service restart`",
            request.client_version
        ));
    }
    serde_json::from_value(request.command)
        .map_err(|error| format!("could not understand the command: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        command_of_a_request_from_a_client_of_the_same_version,
        read_the_json_of_a_request_after_its_length, request_from_this_client,
        write_request_after_its_length,
    };
    use crate::command::DaemonCommand;
    use crate::command::scratchpad::ScratchpadCommand;

    fn a_command() -> DaemonCommand {
        DaemonCommand::Scratchpad(ScratchpadCommand::Toggle {
            name: String::from("terminal"),
        })
    }

    fn framed_request_of(command: DaemonCommand) -> Vec<u8> {
        let mut framed_request = Vec::new();
        write_request_after_its_length(&mut framed_request, &request_from_this_client(command))
            .unwrap();
        framed_request
    }

    #[test]
    fn a_command_read_back_after_its_length_is_the_command_written() {
        let framed_request = framed_request_of(a_command());

        let request_as_json =
            read_the_json_of_a_request_after_its_length(framed_request.as_slice()).unwrap();
        let command_read_back =
            command_of_a_request_from_a_client_of_the_same_version(&request_as_json).unwrap();

        assert_eq!(
            serde_json::to_string(&command_read_back).unwrap(),
            serde_json::to_string(&a_command()).unwrap()
        );
    }

    #[test]
    fn the_length_before_the_request_is_native_endian_and_counts_the_json_bytes() {
        let framed_request = framed_request_of(a_command());

        let (length_bytes, request_as_json) = framed_request.split_at(size_of::<u32>());

        assert_eq!(
            u32::from_ne_bytes(length_bytes.try_into().unwrap()) as usize,
            request_as_json.len()
        );
        assert!(serde_json::from_slice::<serde_json::Value>(request_as_json).is_ok());
    }

    #[test]
    fn a_request_cut_short_is_refused() {
        let mut framed_request = framed_request_of(a_command());
        framed_request.truncate(framed_request.len() - 1);

        assert!(read_the_json_of_a_request_after_its_length(framed_request.as_slice()).is_err());
    }

    #[test]
    fn a_client_of_another_version_is_refused_naming_both_versions_even_with_an_unknown_command() {
        let request_as_json = br#"{"client_version":"0.0.1","command":{"Teleport":{"to":"mars"}}}"#;

        let failure = command_of_a_request_from_a_client_of_the_same_version(request_as_json)
            .err()
            .unwrap();

        assert!(failure.contains("0.0.1"));
        assert!(failure.contains(env!("CARGO_PKG_VERSION")));
        assert!(failure.contains("yabai service restart"));
    }

    #[test]
    fn an_unknown_command_from_a_client_of_the_same_version_is_refused() {
        let request_as_json = format!(
            r#"{{"client_version":"{}","command":{{"Teleport":{{"to":"mars"}}}}}}"#,
            env!("CARGO_PKG_VERSION")
        );

        let failure =
            command_of_a_request_from_a_client_of_the_same_version(request_as_json.as_bytes())
                .err()
                .unwrap();

        assert!(failure.starts_with("could not understand the command"));
    }
}
