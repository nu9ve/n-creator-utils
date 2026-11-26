//! §1: reading and appending the log.

use crate::{MAX_LINE, line_problem, run_problem};
use nu9ve_fs::LockMode;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

/// A line the reader skipped, counted from 1, with its problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogProblem {
    pub line: usize,
    pub problem: &'static str,
}

/// What a reader keeps from a log.
#[derive(Debug, Clone, Default)]
pub struct Log {
    /// Valid runs in order, the first of each `run_id`.
    pub runs: Vec<Value>,
    pub problems: Vec<LogProblem>,
    /// `run_id`s seen again after their first run.
    pub duplicates: Vec<String>,
    /// The text did not end in a newline: its last piece was being written and is ignored.
    pub partial_last_line: bool,
}

/// §1: half-written last line ignored, empty lines skipped, invalid lines reported and skipped, the first run of
/// each `run_id` kept.
pub fn read_log(text: &str) -> Log {
    let mut pieces: Vec<&str> = text.split('\n').collect();
    let partial_last_line = pieces.last().is_some_and(|last| !last.is_empty());
    pieces.pop();
    let mut log = Log {
        partial_last_line,
        ..Log::default()
    };
    let mut seen = HashSet::new();
    for (index, line) in pieces.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(problem) = line_problem(line) {
            log.problems.push(LogProblem {
                line: index + 1,
                problem,
            });
            continue;
        }
        let Ok(run) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let id = run["run_id"].as_str().unwrap_or_default().to_owned();
        if seen.insert(id.clone()) {
            log.runs.push(run);
        } else {
            log.duplicates.push(id);
        }
    }
    log
}

/// §1: validates `run` and appends it as one line, under the exclusive lock of the log file itself.
pub fn append(path: &Path, run: &Value) -> io::Result<()> {
    if let Some(problem) = run_problem(run) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a nu9ve-bench/1 run: {problem}"),
        ));
    }
    let mut line = serde_json::to_string(run).map_err(io::Error::other)?;
    if line.len() > MAX_LINE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a nu9ve-bench/1 run: too_large",
        ));
    }
    line.push('\n');
    let mut file = OpenOptions::new().append(true).create(true).open(path)?;
    nu9ve_fs::lock(&file, LockMode::Exclusive, Duration::from_secs(10))?;
    let written = file.write(line.as_bytes())?;
    if written != line.len() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "short write to the benchmark log",
        ));
    }
    Ok(())
}
