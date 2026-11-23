//! One request, one frame, one connection (nu9ve-engine-v1 §1 and §9; registry §3).

use nu9ve_json::Error;
use serde_json::{Value, json};
use std::time::Duration;

use crate::registry::{Record, Transport};

pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Send one framed request to a registered engine and return the reply frame's bytes.
pub fn call(record: &Record, request: &[u8], timeout: Duration) -> Result<Vec<u8>, Error> {
    if request.is_empty() || request.len() > MAX_FRAME_BYTES {
        return Err(Error::new(
            "request_too_large",
            "The request does not fit in one frame.",
        ));
    }
    match &record.transport {
        #[cfg(unix)]
        Transport::Unix { path } => unix::call(path, request, timeout),
        #[cfg(not(unix))]
        Transport::Unix { .. } => Err(Error::new(
            "engine_unavailable",
            "Unix sockets are not available on this system.",
        )),
        Transport::Pipe { .. } => {
            let _ = timeout;
            Err(Error::new(
                "engine_unavailable",
                "Named pipes are not supported by this client yet.",
            ))
        }
    }
}

/// Send `method` with `params` and return `data`, or the engine's error as it came.
/// The client checks the envelope and that the reply carries the id it sent (engine §2.1, §3).
pub fn request(
    record: &Record,
    method: &str,
    params: Option<Value>,
    idempotency_key: Option<&str>,
    timeout: Duration,
) -> Result<Value, Error> {
    let random = nu9ve_json::random_bytes::<8>()?;
    let id: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let id = format!("nu9ve-{id}");
    let mut message = json!({"id": id, "method": method});
    if let Some(params) = params {
        message["params"] = params;
    }
    if let Some(key) = idempotency_key {
        message["idempotency_key"] = json!(key);
    }
    let bytes =
        serde_json::to_vec(&message).map_err(|e| Error::new("internal_error", e.to_string()))?;
    let reply = call(record, &bytes, timeout)?;
    let invalid = || Error::new("connection_lost", "The engine sent an invalid envelope.");
    let Ok(Value::Object(mut reply)) = serde_json::from_slice::<Value>(&reply) else {
        return Err(invalid());
    };
    if reply.get("id").and_then(Value::as_str) != Some(id.as_str()) {
        return Err(Error::new(
            "connection_lost",
            "The engine answered another request.",
        ));
    }
    match reply.get("ok") {
        Some(Value::Bool(true)) => reply.remove("data").ok_or_else(invalid),
        Some(Value::Bool(false)) => {
            let error = reply.remove("error").ok_or_else(invalid)?;
            let code = error
                .get("code")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            let message = error.get("message").and_then(Value::as_str).unwrap_or("");
            Err(Error::new(code, message))
        }
        _ => Err(invalid()),
    }
}

#[cfg(unix)]
mod unix {
    use super::{Error, MAX_FRAME_BYTES};
    use std::io::{self, Read, Write};
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    use std::os::unix::net::UnixStream;
    use std::path::Path;
    use std::time::Duration;

    pub fn call(path: &Path, request: &[u8], timeout: Duration) -> Result<Vec<u8>, Error> {
        let uid = crate::process::current_uid();
        // Before connecting: a socket of this user with no group or other permissions.
        let meta = std::fs::symlink_metadata(path)
            .map_err(|_| Error::new("engine_unavailable", "The engine socket is not there."))?;
        if !meta.file_type().is_socket() || Some(meta.uid()) != uid || meta.mode() & 0o077 != 0 {
            return Err(Error::new(
                "unsafe_endpoint",
                "The engine socket is not a private socket of this user.",
            ));
        }
        let mut stream = UnixStream::connect(path)
            .map_err(|_| Error::new("engine_unavailable", "The engine does not answer."))?;
        if peer_uid(&stream).ok() != uid {
            return Err(Error::new("peer_user", "The engine runs as another user."));
        }
        let io_error = |e: io::Error| {
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) {
                Error::new("timeout", "The engine did not answer in time.")
            } else {
                Error::new("connection_lost", "The connection to the engine closed.")
            }
        };
        stream.set_read_timeout(Some(timeout)).map_err(io_error)?;
        stream.set_write_timeout(Some(timeout)).map_err(io_error)?;
        let size = u32::try_from(request.len()).map_err(|_| {
            Error::new(
                "request_too_large",
                "The request does not fit in one frame.",
            )
        })?;
        stream.write_all(&size.to_be_bytes()).map_err(io_error)?;
        stream.write_all(request).map_err(io_error)?;
        let mut header = [0; 4];
        stream.read_exact(&mut header).map_err(io_error)?;
        let size = u32::from_be_bytes(header) as usize;
        if size == 0 || size > MAX_FRAME_BYTES {
            return Err(Error::new(
                "connection_lost",
                "The engine sent a frame of the wrong size.",
            ));
        }
        let mut reply = vec![0; size];
        stream.read_exact(&mut reply).map_err(io_error)?;
        Ok(reply)
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd"))]
    fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
        use std::os::fd::AsRawFd;
        let (mut uid, mut gid) = (0, 0);
        // SAFETY: valid socket descriptor and two live out-parameters.
        if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(uid)
    }

    #[cfg(target_os = "linux")]
    fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
        use std::os::fd::AsRawFd;
        // SAFETY: an all-zero ucred is a valid value of this plain C struct.
        let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: valid descriptor; the buffer is a live ucred of `size` bytes.
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&raw mut cred).cast(),
                &mut size,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(cred.uid)
    }

    #[cfg(not(any(target_os = "macos", target_os = "freebsd", target_os = "linux")))]
    fn peer_uid(_: &UnixStream) -> io::Result<u32> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}
