//! Local RPC transport and request handling for nu9ve-engine-v1.

mod transport;

use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::value::RawValue;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

pub use transport::{MAX_MESSAGE_BYTES, serve_frames, serve_lines};

pub const CONTRACT: &str = "nu9ve-engine/1";

/// Process metadata, including the app's ordered, disjoint method lists.
pub struct EngineInfo<'a> {
    pub protocol: &'a str,
    pub version: &'a str,
    pub capabilities: &'a [&'a str],
    pub ported: &'a [&'a str],
    pub native: &'a [&'a str],
    pub extra: Map<String, Value>,
}

impl EngineInfo<'_> {
    /// Build the handshake data. Contract fields take precedence over app fields.
    pub fn into_data(self) -> Value {
        let mut fields = self.extra;
        let Value::Object(core) = json!({
            "contract": CONTRACT,
            "protocol": self.protocol,
            "engine": "rust",
            "version": self.version,
            "capabilities": self.capabilities,
            "ported": self.ported,
            "native": self.native,
        }) else {
            unreachable!("handshake is an object");
        };
        fields.extend(core);
        Value::Object(fields)
    }
}

pub struct Request<'a> {
    pub method: &'a str,
    pub params: &'a Map<String, Value>,
    /// The app decides which methods require a key and validates it for those methods.
    pub idempotency_key: Option<String>,
}

pub enum Reply {
    Data(Value),
    /// An already serialized JSON value. The server checks the line and frame constraints.
    Encoded(Box<RawValue>),
}

impl From<Value> for Reply {
    fn from(value: Value) -> Self {
        Self::Data(value)
    }
}

pub struct RpcError {
    error: nu9ve_json::Error,
    reason: Option<&'static str>,
}

impl RpcError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            error: nu9ve_json::Error::new(code, message),
            reason: None,
        }
    }

    /// Use a reason token from the contract or the app. The request must have had no effects.
    pub fn not_ported(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason: Some(reason),
            ..Self::new("not_ported", message)
        }
    }

    /// The error as the client will see it: its `code` and `message`.
    pub fn error(&self) -> &nu9ve_json::Error {
        &self.error
    }
}

/// `code: message`, for a caller that runs a method in process instead of over the transport.
impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error.code, self.error.message)
    }
}

pub type Result = std::result::Result<Reply, RpcError>;

struct Response<'a> {
    id: &'a RawValue,
    envelope: Value,
    encoded: Option<Box<RawValue>>,
}

impl<'a> Response<'a> {
    fn new(id: &'a RawValue, result: Result) -> Self {
        let mut encoded = None;
        let envelope = match result {
            Ok(Reply::Data(data)) => nu9ve_json::ok_envelope(&Value::Null, data),
            Ok(Reply::Encoded(data)) => {
                encoded = Some(data);
                nu9ve_json::ok_envelope(&Value::Null, Value::Null)
            }
            Err(error) => {
                let mut value = nu9ve_json::error_envelope(&Value::Null, &error.error);
                if let Some(reason) = error.reason {
                    value["error"]["reason"] = json!(reason);
                }
                value
            }
        };
        Self {
            id,
            envelope,
            encoded,
        }
    }

    fn error(id: &'a RawValue, code: &str, message: &str) -> Self {
        Self::new(id, Err(RpcError::new(code, message)))
    }
}

impl Serialize for Response<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let fields = self.envelope.as_object().expect("response envelope");
        let mut map = serializer.serialize_map(Some(fields.len()))?;
        // Write id first in both feature configurations.
        map.serialize_entry("id", self.id)?;
        for (key, value) in fields {
            match (key.as_str(), &self.encoded) {
                ("id", _) => {}
                ("data", Some(raw)) => map.serialize_entry(key, raw)?,
                _ => map.serialize_entry(key, value)?,
            }
        }
        map.end()
    }
}

fn valid_id(id: &RawValue) -> bool {
    let text = id.get();
    match text.as_bytes().first() {
        Some(b'"') => serde_json::from_str::<String>(text).is_ok(),
        // RawValue already validated the number grammar, including leading zeroes.
        Some(b'-' | b'0'..=b'9') => !text.contains(['.', 'e', 'E']),
        _ => text == "null",
    }
}

fn answer<'a>(
    payload: &'a [u8],
    info: &Value,
    dispatch: &mut impl FnMut(Request<'_>) -> Result,
) -> Response<'a> {
    // Borrow raw fields to keep the id token intact independently of serde_json's number features.
    let invalid_json = || {
        Response::error(
            RawValue::NULL,
            "invalid_request",
            "Expected valid UTF-8 JSON.",
        )
    };
    let Ok(text) = std::str::from_utf8(payload) else {
        return invalid_json();
    };
    if !text
        .trim_start_matches([' ', '\t', '\n', '\r'])
        .starts_with('{')
    {
        return if serde_json::from_str::<&RawValue>(text).is_ok() {
            Response::error(
                RawValue::NULL,
                "invalid_request",
                "Expected a request object.",
            )
        } else {
            invalid_json()
        };
    }
    let Ok(request) = serde_json::from_str::<HashMap<String, &RawValue>>(text) else {
        return invalid_json();
    };
    let id = request.get("id").copied().unwrap_or(RawValue::NULL);
    if !valid_id(id) {
        return Response::error(
            RawValue::NULL,
            "invalid_request",
            "Expected an integer, string or null id.",
        );
    }
    let Some(method) = request
        .get("method")
        .and_then(|raw| serde_json::from_str::<String>(raw.get()).ok())
    else {
        return Response::error(id, "invalid_request", "Expected a string method.");
    };
    let params = match request.get("params") {
        None => Map::new(),
        Some(raw) if raw.get().starts_with('{') => {
            let Ok(params) = serde_json::from_str(raw.get()) else {
                return invalid_json();
            };
            params
        }
        Some(_) => return Response::error(id, "invalid_params", "Expected a params object."),
    };
    let result = if method == "engine_info" {
        if params.is_empty() {
            Ok(info.clone().into())
        } else {
            Err(RpcError::new(
                "invalid_params",
                "This method accepts an empty params object.",
            ))
        }
    } else {
        dispatch(Request {
            method: &method,
            params: &params,
            idempotency_key: request
                .get("idempotency_key")
                .and_then(|raw| serde_json::from_str(raw.get()).ok()),
        })
    };
    Response::new(id, result)
}

#[cfg(test)]
mod tests;
