pub(crate) const FAILURE_RESPONSE_MARKER: &[u8] = b"\x07";

enum ResponseSink {
    Client(std::io::BufWriter<std::os::unix::net::UnixStream>),
    StandardOutput(std::io::Stdout),
    Silent,
}

pub struct Response {
    sink: ResponseSink,
}

impl Response {
    pub fn to_client(stream: std::os::unix::net::UnixStream) -> Response {
        Response {
            sink: ResponseSink::Client(std::io::BufWriter::new(stream)),
        }
    }

    pub fn to_standard_output() -> Response {
        Response {
            sink: ResponseSink::StandardOutput(std::io::stdout()),
        }
    }

    pub fn silent() -> Response {
        Response {
            sink: ResponseSink::Silent,
        }
    }

    fn is_silent(&self) -> bool {
        matches!(self.sink, ResponseSink::Silent)
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        let _ = match &mut self.sink {
            ResponseSink::Client(stream) => std::io::Write::write_all(stream, bytes),
            ResponseSink::StandardOutput(standard_output) => {
                std::io::Write::write_all(standard_output, bytes)
            }
            ResponseSink::Silent => Ok(()),
        };
    }

    pub fn write_bytes_stopping_at_first_null(&mut self, bytes: &[u8]) {
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        self.write_bytes(&bytes[..end]);
    }

    pub fn write(&mut self, arguments: std::fmt::Arguments) {
        let _ = match &mut self.sink {
            ResponseSink::Client(stream) => std::io::Write::write_fmt(stream, arguments),
            ResponseSink::StandardOutput(standard_output) => {
                std::io::Write::write_fmt(standard_output, arguments)
            }
            ResponseSink::Silent => Ok(()),
        };
    }

    pub fn write_failure_marker(&mut self) {
        self.write_bytes(FAILURE_RESPONSE_MARKER);
    }

    pub fn write_failure_unless_silent(&mut self, arguments: std::fmt::Arguments) {
        if self.is_silent() {
            return;
        }
        self.write_failure_marker();
        self.write(arguments);
    }

    pub fn write_failure_pieces_unless_silent(&mut self, pieces: &[FailurePiece]) {
        if self.is_silent() {
            return;
        }
        self.write_failure_marker();
        for piece in pieces {
            match piece {
                FailurePiece::Text(text) => self.write_bytes(text.as_bytes()),
                FailurePiece::Bytes(bytes) => self.write_bytes(bytes),
                FailurePiece::BytesStoppingAtFirstNull(bytes) => {
                    self.write_bytes_stopping_at_first_null(bytes)
                }
            }
        }
    }
}

pub enum FailurePiece<'message> {
    Text(&'message str),
    Bytes(&'message [u8]),
    BytesStoppingAtFirstNull(&'message [u8]),
}

impl Drop for Response {
    fn drop(&mut self) {
        let _ = match &mut self.sink {
            ResponseSink::Client(stream) => std::io::Write::flush(stream),
            ResponseSink::StandardOutput(standard_output) => std::io::Write::flush(standard_output),
            ResponseSink::Silent => Ok(()),
        };
    }
}

#[macro_export]
macro_rules! daemon_fail {
    ($response:expr, $($argument:tt)*) => {
        $response.write_failure_unless_silent(format_args!($($argument)*))
    };
}
