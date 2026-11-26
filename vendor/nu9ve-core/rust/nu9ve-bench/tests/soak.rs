//! §5.1 and §5.2 (minor version of 2026-11-25): the memory slope and the halving of a long series against
//! data/bench-examples-v1.json, and real soaks in this process. The soaks run one at a time: they read this
//! process's memory, which a parallel test would move.

use nu9ve_bench::measure::{
    ProcessSample, RunInfo, Soak, measure_soak, new_run, process_sample, resources,
};
use nu9ve_bench::{MAX_LINE, SOAK_GROWTH, SOAK_SLOPE, memory_slope, run_problem};
use serde_json::{Value, json};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Mutex;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn examples() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bench-examples-v1.json");
    serde_json::from_str(&fs::read_to_string(path).expect("examples")).expect("JSON")
}

fn floats(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}

#[test]
fn slopes_match_the_reference_bit_for_bit() {
    let examples = examples();
    let cases = examples["slopes"].as_array().unwrap();
    assert!(cases.len() >= 6);
    for case in cases {
        let got = memory_slope(&floats(&case["iterations"]), &floats(&case["rss_bytes"]));
        let want = case["slope"].as_f64();
        assert_eq!(
            got.map(f64::to_bits),
            want.map(f64::to_bits),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn long_series_are_halved_like_the_reference() {
    for case in examples()["reductions"].as_array().unwrap() {
        let samples: Vec<ProcessSample> = case["samples"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| ProcessSample {
                rss_bytes: s["rss_bytes"].as_u64().unwrap(),
                cpu_ms: s["cpu_ms"].as_u64().unwrap(),
            })
            .collect();
        let got = resources(
            case["interval_ms"].as_u64().unwrap(),
            &samples,
            case["max_samples"].as_u64().unwrap() as usize,
        );
        assert_eq!(got, case["result"], "{}", case["name"]);
    }
}

fn info(case: &str) -> RunInfo {
    RunInfo {
        suite: "core-selftest".into(),
        case: case.into(),
        category: "soak".into(),
        subject: json!({"app": "core", "engine": "rust", "version": "0.1.0", "commit": null, "dirty": null}),
        machine: None,
        inputs: vec![],
        params: None,
        cache: "warm",
        privacy: "public",
        quality: None,
        note: None,
    }
}

fn quality<'a>(quality: &'a Value, metric: &str) -> &'a Value {
    quality
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["metric"] == metric)
        .unwrap()
}

#[test]
fn this_process_can_be_sampled() {
    let sample = process_sample(None).expect("this process");
    assert!(sample.rss_bytes > 1 << 20, "{sample:?}");
}

#[test]
fn a_soak_without_a_leak_passes_and_makes_a_valid_run() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut soak = Soak::new(4000, 400, 512.0);
    soak.interval_ms = 2;
    let soaked = measure_soak(&soak, || {
        // Allocates, touches and frees: memory comes back to the allocator every iteration.
        let block = vec![7u8; 16 * 1024];
        std::hint::black_box(&block);
        Ok(())
    })
    .unwrap();
    assert_eq!(soaked.measured.status, "ok");
    let slope = quality(&soaked.quality, SOAK_SLOPE);
    assert_eq!(slope["op"], "le");
    assert_eq!(slope["pass"], true, "{slope}");
    assert!(quality(&soaked.quality, SOAK_GROWTH)["value"].is_i64());
    assert_eq!(soaked.params["iterations"], 4000);
    assert_eq!(soaked.params["warmup_iterations"], 400);

    let metrics = soaked.measured.metrics.clone().unwrap();
    let series = &metrics["resources"];
    let length = series["rss_bytes"].as_array().unwrap().len();
    assert!(length >= 2 && length <= soak.max_samples);
    assert_eq!(series["cpu_ms"].as_array().unwrap().len(), length);
    assert_eq!(metrics["wall_ms"]["samples"].as_array().unwrap().len(), 1);
    assert_eq!(metrics["throughput"]["unit"], "iter/s");

    let mut info = info("rust/no-leak");
    info.params = Some(soaked.params);
    info.quality = Some(soaked.quality);
    let run = new_run(info, soaked.measured).expect("a valid soak run");
    assert_eq!(run_problem(&run), None);
    assert!(serde_json::to_string(&run).unwrap().len() <= MAX_LINE);
}

#[test]
fn a_leak_fails_its_slope_limit() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut soak = Soak::new(1500, 100, 1024.0);
    soak.interval_ms = 2;
    let soaked = measure_soak(&soak, || {
        // 16 KiB written and never freed: about 16 384 resident bytes per iteration.
        let block: &'static mut [u8] = Box::leak(vec![1u8; 16 * 1024].into_boxed_slice());
        std::hint::black_box(block);
        Ok(())
    })
    .unwrap();
    let slope = quality(&soaked.quality, SOAK_SLOPE);
    assert_eq!(slope["pass"], false, "{slope}");
    assert!(slope["value"].as_f64().unwrap() > 8192.0, "{slope}");
    assert!(
        quality(&soaked.quality, SOAK_GROWTH)["value"]
            .as_i64()
            .unwrap()
            > 8 << 20
    );
    let mut info = info("rust/leak");
    info.quality = Some(soaked.quality);
    assert_eq!(run_problem(&new_run(info, soaked.measured).unwrap()), None);
}

#[test]
fn a_long_soak_keeps_the_line_small() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut soak = Soak::new(20_000, 0, 4096.0);
    soak.interval_ms = 1;
    soak.max_samples = 40;
    let soaked = measure_soak(&soak, || {
        std::hint::black_box((0..2_000u64).sum::<u64>());
        Ok(())
    })
    .unwrap();
    let series = &soaked.measured.metrics.unwrap()["resources"];
    assert!(series["rss_bytes"].as_array().unwrap().len() <= 40);
    assert!(series["interval_ms"].as_u64().unwrap() >= 1);
}

#[test]
fn a_failing_step_gives_a_failed_run() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut count = 0;
    let soaked = measure_soak(&Soak::new(100, 10, 64.0), || {
        count += 1;
        if count == 25 {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        } else {
            Ok(())
        }
    })
    .unwrap();
    assert_eq!(soaked.measured.status, "failed");
    let error = soaked.measured.error.clone().unwrap();
    assert_eq!(error["code"], "step_failed");
    assert_eq!(error["message"], "iteration 14 failed: BrokenPipe");
    let run = new_run(info("rust/broken"), soaked.measured).unwrap();
    assert_eq!(run_problem(&run), None);
}

#[test]
fn a_soak_needs_iterations_and_a_finite_limit() {
    assert!(measure_soak(&Soak::new(0, 0, 64.0), || Ok(())).is_err());
    assert!(measure_soak(&Soak::new(10, 0, f64::NAN), || Ok(())).is_err());
}

#[cfg(unix)]
#[test]
fn another_process_is_sampled_until_it_ends() {
    use std::process::{Command, Stdio};
    let mut child = Command::new("sleep")
        .arg("5")
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let sample = process_sample(Some(child.id()));
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(sample.expect("a running child").rss_bytes > 0);
    assert_eq!(process_sample(Some(child.id())), None);
}
