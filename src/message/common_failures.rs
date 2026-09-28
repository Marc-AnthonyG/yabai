use crate::message::token::Token;
use crate::support::response::{FailurePiece, Response};

pub(crate) fn daemon_fail_with_unknown_value_given_to_command_for_domain(
    response: &mut Response,
    message_bytes: &[u8],
    value: Token,
    command: Token,
    domain: Token,
) {
    response.write_failure_pieces_unless_silent(&[
        FailurePiece::Text("unknown value '"),
        FailurePiece::Bytes(value.bytes(message_bytes)),
        FailurePiece::Text("' given to command '"),
        FailurePiece::Bytes(command.bytes(message_bytes)),
        FailurePiece::Text("' for domain '"),
        FailurePiece::Bytes(domain.bytes(message_bytes)),
        FailurePiece::Text("'\n"),
    ]);
}

pub(crate) fn daemon_fail_with_unknown_command_for_domain(
    response: &mut Response,
    message_bytes: &[u8],
    command: Token,
    domain: Token,
) {
    response.write_failure_pieces_unless_silent(&[
        FailurePiece::Text("unknown command '"),
        FailurePiece::Bytes(command.bytes(message_bytes)),
        FailurePiece::Text("' for domain '"),
        FailurePiece::Bytes(domain.bytes(message_bytes)),
        FailurePiece::Text("'\n"),
    ]);
}
