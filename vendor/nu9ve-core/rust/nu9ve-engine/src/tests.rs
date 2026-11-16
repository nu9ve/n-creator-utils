use super::*;
use std::io::{BufReader, Cursor, Read};

fn info() -> EngineInfo<'static> {
    EngineInfo {
        protocol: "test/1",
        version: "0.1.2",
        capabilities: &["echo_v1"],
        ported: &["ported"],
        native: &["engine_info", "echo"],
        extra: Map::from_iter([
            ("contract".into(), json!("app cannot override this")),
            ("revision".into(), json!(2)),
        ]),
    }
}

fn dispatch(request: Request<'_>) -> Result {
    match request.method {
        "ported" => Err(RpcError::not_ported("platform_unavailable", "App message.")),
        "echo" => Ok(json!({"params": request.params, "key": request.idempotency_key}).into()),
        "large" => Ok(json!("x".repeat(MAX_MESSAGE_BYTES)).into()),
        "encoded" => Ok(Reply::Encoded(
            RawValue::from_string("1.00".into()).unwrap(),
        )),
        "multiline" => Ok(Reply::Encoded(
            RawValue::from_string("[\n1]".into()).unwrap(),
        )),
        _ => Err(RpcError::new("unknown_method", "Unknown method.")),
    }
}

fn frame(output: &mut Vec<u8>, payload: &[u8]) {
    output.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    output.extend_from_slice(payload);
}

fn frames(output: &[u8]) -> Vec<&str> {
    let mut cursor = Cursor::new(output);
    let mut payloads = Vec::new();
    while (cursor.position() as usize) < output.len() {
        let mut header = [0; 4];
        cursor.read_exact(&mut header).unwrap();
        let start = cursor.position() as usize;
        let end = start + u32::from_be_bytes(header) as usize;
        assert!(end > start && end - start <= MAX_MESSAGE_BYTES);
        payloads.push(std::str::from_utf8(&output[start..end]).unwrap());
        cursor.set_position(end as u64);
    }
    payloads
}

#[test]
fn contract_examples() {
    let vectors: Value =
        serde_json::from_str(include_str!("../../../data/engine-examples-v1.json")).unwrap();
    for framed in [false, true] {
        let mut input = Vec::new();
        let mut expected = Vec::new();
        for case in vectors["server"].as_array().unwrap() {
            let req = case["request"].as_str().unwrap();
            let req = if let Some(pad) = case["pad_bytes"].as_u64() {
                req.replace("<pad>", &"a".repeat(pad as usize))
            } else {
                req.to_owned()
            };
            if framed {
                frame(&mut input, req.as_bytes());
            } else {
                input.extend_from_slice(req.as_bytes());
                input.push(b'\n');
            }
            if case["response"].is_null() {
                if framed {
                    expected.push(json!({"id": "null", "ok": false, "code": "invalid_request"}));
                }
            } else {
                expected.push(case["response"].clone());
            }
            if let Some(next) = case.get("then") {
                let req = next["request"].as_str().unwrap();
                if framed {
                    frame(&mut input, req.as_bytes());
                } else {
                    input.extend_from_slice(req.as_bytes());
                    input.push(b'\n');
                }
                expected.push(next["response"].clone());
            }
        }
        let mut output = Vec::new();
        let input = BufReader::with_capacity(31, input.as_slice());
        let replies: Vec<&str> = if framed {
            serve_frames(input, &mut output, info(), dispatch).unwrap();
            frames(&output)
        } else {
            serve_lines(input, &mut output, info(), dispatch).unwrap();
            std::str::from_utf8(&output).unwrap().lines().collect()
        };
        assert_eq!(replies.len(), expected.len());
        for (reply, want) in replies.into_iter().zip(expected) {
            let parsed: HashMap<String, &RawValue> = serde_json::from_str(reply).unwrap();
            assert_eq!(parsed["id"].get(), want["id"].as_str().unwrap(), "{reply}");
            let body: Value = serde_json::from_str(reply).unwrap();
            assert_eq!(body["ok"], want["ok"]);
            if want["ok"] == true {
                assert_eq!(body["data"], info().into_data());
                assert!(body.get("error").is_none());
            } else {
                assert_eq!(body["error"]["code"], want["code"]);
                assert!(body["error"]["message"].is_string());
                assert!(body.get("data").is_none());
            }
        }
    }
}

#[test]
fn exact_id_and_app_dispatch() {
    let big = format!("-1{}", "2".repeat(400));
    let input = format!(
        "{{\"id\":{big},\"method\":\"ported\"}}\n\
         {{\"id\":-0,\"method\":\"echo\",\"params\":{{\"b\":1,\"a\":2}},\"idempotency_key\":\"abc\"}}\n\
         {{\"method\":\"encoded\"}}\n{{\"method\":\"multiline\"}}"
    );
    let mut output = Vec::new();
    serve_lines(input.as_bytes(), &mut output, info(), dispatch).unwrap();
    let output = std::str::from_utf8(&output).unwrap();
    let rows: Vec<HashMap<String, &RawValue>> = output
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows[0]["id"].get(), big);
    let error: Value = serde_json::from_str(rows[0]["error"].get()).unwrap();
    assert_eq!(
        error,
        json!({"code":"not_ported","reason":"platform_unavailable","message":"App message."})
    );
    assert_eq!(rows[1]["id"].get(), "-0");
    let params = if cfg!(feature = "preserve_order") {
        r#"{"params":{"b":1,"a":2},"key":"abc"}"#
    } else {
        r#"{"key":"abc","params":{"a":2,"b":1}}"#
    };
    assert_eq!(rows[1]["data"].get(), params);
    assert_eq!(rows[2]["data"].get(), "1.00");
    let error: Value = serde_json::from_str(rows[3]["error"].get()).unwrap();
    assert_eq!(error["code"], "internal_error");
}

#[test]
fn frame_limits_and_recovery() {
    let mut input = Vec::new();
    frame(&mut input, b"");
    let request = br#"{"id":"large","method":"large"}"#;
    let mut boundary = request.to_vec();
    boundary.resize(MAX_MESSAGE_BYTES, b' ');
    frame(&mut input, &boundary);
    frame(&mut input, br#"{"id":"next","method":"encoded"}"#);
    let mut output = Vec::new();
    serve_frames(input.as_slice(), &mut output, info(), dispatch).unwrap();
    let replies: Vec<Value> = frames(&output)
        .into_iter()
        .map(|text| serde_json::from_str(text).unwrap())
        .collect();
    assert_eq!(replies.len(), 3);
    assert_eq!(replies[0]["error"]["code"], "request_too_large");
    assert_eq!(replies[1]["id"], "large");
    assert_eq!(replies[1]["error"]["code"], "response_too_large");
    assert_eq!(replies[2]["id"], "next");
    assert_eq!(replies[2]["ok"], true);
}

#[test]
fn oversized_line_ending_at_read_limit_keeps_next_request() {
    let mut input = vec![b' '; MAX_MESSAGE_BYTES];
    input.extend_from_slice(b"\n{\"id\":7,\"method\":\"echo\"}\n");
    let mut output = Vec::new();
    serve_lines(input.as_slice(), &mut output, info(), dispatch).unwrap();
    let replies: Vec<Value> = std::str::from_utf8(&output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["error"]["code"], "request_too_large");
    assert_eq!(replies[1]["id"], 7);
}

#[test]
fn closed_response_pipe_ends_the_session() {
    use std::io::{self, Write};

    struct Closed;

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let payload = br#"{"method":"engine_info"}"#;
    serve_lines(payload.as_slice(), Closed, info(), dispatch).unwrap();
    let mut framed = Vec::new();
    frame(&mut framed, payload);
    serve_frames(framed.as_slice(), Closed, info(), dispatch).unwrap();
}
