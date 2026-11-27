//! nu9ve-time against the `time` section of data/py-vectors-v1.json, written by CPython 3.14.
use nu9ve_time::DateTime;
use serde_json::Value;

fn vectors() -> Value {
    let all: Value =
        serde_json::from_str(include_str!("../../../data/py-vectors-v1.json")).unwrap();
    all["time"].clone()
}

#[test]
fn fromisoformat_like_python() {
    for case in vectors()["fromisoformat"].as_array().unwrap() {
        let got = DateTime::fromisoformat(case["text"].as_str().unwrap()).map(|d| d.isoformat());
        if case["refused"] == true {
            assert_eq!(got, None, "{case}");
        } else {
            assert_eq!(got.as_deref(), case["isoformat"].as_str(), "{case}");
        }
    }
}

#[test]
fn fromtimestamp_utc_like_python() {
    for case in vectors()["fromtimestamp_utc"].as_array().unwrap() {
        let t = f64::from_bits(u64::from_str_radix(case["t"].as_str().unwrap(), 16).unwrap());
        assert_eq!(
            DateTime::fromtimestamp_utc(t).isoformat(),
            case["isoformat"].as_str().unwrap(),
            "{case}"
        );
    }
}

#[test]
fn strftime_like_python() {
    for case in vectors()["strftime"].as_array().unwrap() {
        let value = DateTime::fromisoformat(case["text"].as_str().unwrap()).unwrap();
        assert_eq!(
            value.strftime(case["pattern"].as_str().unwrap()),
            case["result"].as_str().unwrap(),
            "{case}"
        );
    }
}
