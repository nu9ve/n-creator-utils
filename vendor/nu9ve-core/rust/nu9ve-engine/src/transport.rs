use crate::{EngineInfo, Request, Response, Result, answer};
use serde_json::value::RawValue;
use std::io::{self, BufRead, Read, Write};

pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

fn oversized_request() -> Response<'static> {
    Response::error(
        RawValue::NULL,
        "request_too_large",
        "Request exceeds 1 MiB.",
    )
}

fn disconnected(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result,
    }
}

/// Serve NDJSON until EOF. An oversized line is discarded, and stdout closing
/// (BrokenPipe) ends the session successfully.
pub fn serve_lines(
    mut input: impl BufRead,
    mut output: impl Write,
    info: EngineInfo<'_>,
    mut dispatch: impl FnMut(Request<'_>) -> Result,
) -> io::Result<()> {
    let info = info.into_data();
    let mut line = Vec::new();
    disconnected((|| {
        loop {
            line.clear();
            if (&mut input)
                .take((MAX_MESSAGE_BYTES + 1) as u64)
                .read_until(b'\n', &mut line)?
                == 0
            {
                return Ok(());
            }
            let response = if line.len() > MAX_MESSAGE_BYTES {
                serde_json::to_writer(&mut output, &oversized_request())?;
                output.write_all(b"\n")?;
                output.flush()?;
                if !line.ends_with(b"\n") {
                    input.skip_until(b'\n')?;
                }
                continue;
            } else if line
                .iter()
                .all(|byte| byte.is_ascii_whitespace() || *byte == b'\x0b')
            {
                continue;
            } else {
                answer(&line, &info, &mut dispatch)
            };
            let response = match response.encoded.as_deref() {
                Some(raw) if raw.get().contains(['\n', '\r']) => Response::error(
                    response.id,
                    "internal_error",
                    "Expected single-line response data.",
                ),
                _ => response,
            };
            serde_json::to_writer(&mut output, &response)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    })())
}

/// Answer one request in this process, without a stream: for an engine linked into its app (an FFI) that
/// still speaks nu9ve-engine-v1. The rules are those of the stream modes: the 1 MiB limit, the exact id,
/// the envelope and `engine_info`. Returns the compact envelope, without a trailing newline. A blank
/// request is `invalid_request`: there is no stream to skip it in. Encoded data may span lines here, since
/// nothing frames the reply.
pub fn respond(
    request: &[u8],
    info: EngineInfo<'_>,
    mut dispatch: impl FnMut(Request<'_>) -> Result,
) -> Vec<u8> {
    let response = if request.len() > MAX_MESSAGE_BYTES {
        oversized_request()
    } else {
        answer(request, &info.into_data(), &mut dispatch)
    };
    serde_json::to_vec(&response).unwrap_or_else(|_| {
        br#"{"id":null,"ok":false,"error":{"code":"internal_error","message":"Could not encode the response."}}"#.to_vec()
    })
}

/// Serve successive frames on a stream. Each frame has a four-byte, unsigned
/// big-endian length followed by UTF-8 JSON. An oversized frame's body is
/// drained with bounded memory before the next header is read.
pub fn serve_frames(
    mut input: impl BufRead,
    mut output: impl Write,
    info: EngineInfo<'_>,
    mut dispatch: impl FnMut(Request<'_>) -> Result,
) -> io::Result<()> {
    let info = info.into_data();
    let mut payload = Vec::new();
    let mut encoded = FrameBuffer::default();
    disconnected((|| {
        loop {
            let mut header = [0; 4];
            if input.fill_buf()?.is_empty() {
                return Ok(());
            }
            input.read_exact(&mut header)?;
            let size = u32::from_be_bytes(header);
            if size == 0 || size as usize > MAX_MESSAGE_BYTES {
                let response = if size == 0 {
                    Response::error(
                        RawValue::NULL,
                        "request_too_large",
                        "Frame must not be empty.",
                    )
                } else {
                    oversized_request()
                };
                write_frame(&mut output, &mut encoded, &response)?;
                let drained = io::copy(&mut (&mut input).take(u64::from(size)), &mut io::sink())?;
                if drained < u64::from(size) {
                    return Ok(());
                }
                continue;
            }
            payload.resize(size as usize, 0);
            input.read_exact(&mut payload)?;
            let response = answer(&payload, &info, &mut dispatch);
            write_frame(&mut output, &mut encoded, &response)?;
        }
    })())
}

#[derive(Default)]
struct FrameBuffer {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl FrameBuffer {
    fn encode(&mut self, response: &Response<'_>) -> io::Result<bool> {
        self.bytes.clear();
        self.exceeded = false;
        match serde_json::to_writer(&mut *self, response) {
            Ok(()) => Ok(true),
            Err(_) if self.exceeded => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

impl Write for FrameBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_MESSAGE_BYTES - self.bytes.len() {
            self.exceeded = true;
            return Err(io::Error::other("frame exceeds 1 MiB"));
        }
        self.bytes.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn write_frame(
    output: &mut impl Write,
    buffer: &mut FrameBuffer,
    response: &Response<'_>,
) -> io::Result<()> {
    if !buffer.encode(response)? {
        let code = "response_too_large";
        let message = "Response exceeds 1 MiB.";
        if !buffer.encode(&Response::error(response.id, code, message))? {
            // A request id can fill nearly the whole frame. Echo it whenever an error fits.
            buffer.encode(&Response::error(RawValue::NULL, code, message))?;
        }
    }
    output.write_all(&(buffer.bytes.len() as u32).to_be_bytes())?;
    output.write_all(&buffer.bytes)?;
    output.flush()
}
