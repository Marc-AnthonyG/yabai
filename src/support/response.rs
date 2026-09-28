pub struct Response {
    standard_output: Vec<u8>,
    failure_text: Vec<u8>,
    has_begun_writing_failures: bool,
    is_silent: bool,
}

impl Response {
    pub fn collecting() -> Response {
        Response {
            standard_output: Vec::new(),
            failure_text: Vec::new(),
            has_begun_writing_failures: false,
            is_silent: false,
        }
    }

    pub fn silent() -> Response {
        Response {
            is_silent: true,
            ..Response::collecting()
        }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        if self.has_begun_writing_failures {
            self.failure_text.extend_from_slice(bytes);
        } else {
            self.standard_output.extend_from_slice(bytes);
        }
    }

    pub fn write(&mut self, arguments: std::fmt::Arguments) {
        self.write_bytes(std::fmt::format(arguments).as_bytes());
    }

    pub fn write_failure_marker(&mut self) {
        self.has_begun_writing_failures = true;
    }

    pub fn write_failure_unless_silent(&mut self, arguments: std::fmt::Arguments) {
        if self.is_silent {
            return;
        }
        self.write_failure_marker();
        self.write(arguments);
    }

    pub fn write_failure_pieces_unless_silent(&mut self, pieces: &[FailurePiece]) {
        if self.is_silent {
            return;
        }
        self.write_failure_marker();
        for piece in pieces {
            match piece {
                FailurePiece::Text(text) => self.write_bytes(text.as_bytes()),
                FailurePiece::Bytes(bytes) => self.write_bytes(bytes),
            }
        }
    }

    pub fn into_standard_output_and_one_failure_per_line(self) -> (String, Vec<String>) {
        let standard_output = String::from_utf8_lossy(&self.standard_output).into_owned();
        let failures = String::from_utf8_lossy(&self.failure_text)
            .lines()
            .map(String::from)
            .collect();
        (standard_output, failures)
    }
}

pub enum FailurePiece<'message> {
    Text(&'message str),
    Bytes(&'message [u8]),
}

#[macro_export]
macro_rules! daemon_fail {
    ($response:expr, $($argument:tt)*) => {
        $response.write_failure_unless_silent(format_args!($($argument)*))
    };
}

#[cfg(test)]
mod tests {
    use super::{FailurePiece, Response};

    #[test]
    fn output_before_a_failure_is_standard_output_and_each_failure_line_is_one_failure() {
        let mut response = Response::collecting();
        response.write(format_args!("[]\n"));
        crate::daemon_fail!(response, "could not locate the selected window.\n");
        response.write_failure_pieces_unless_silent(&[
            FailurePiece::Text("value '"),
            FailurePiece::Bytes(b"west"),
            FailurePiece::Text("' is not a valid option for WINDOW_SEL\n"),
        ]);

        let (standard_output, failures) = response.into_standard_output_and_one_failure_per_line();

        assert_eq!(standard_output, "[]\n");
        assert_eq!(
            failures,
            [
                "could not locate the selected window.",
                "value 'west' is not a valid option for WINDOW_SEL"
            ]
        );
    }

    #[test]
    fn a_silent_response_keeps_no_failure() {
        let mut response = Response::silent();
        crate::daemon_fail!(response, "could not locate the selected window.\n");

        let (_, failures) = response.into_standard_output_and_one_failure_per_line();
        assert!(failures.is_empty());
    }
}
