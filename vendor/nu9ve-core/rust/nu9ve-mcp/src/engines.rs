//! Asking a live engine for what the suite reads from it: its catalog, page by page (nu9ve-data-v1 §5).
//! Shared by `nu9ve mcp` and `nu9ve agent`.

use crate::catalog::{self, Reply};
use nu9ve_ipc::{DEFAULT_TIMEOUT, Record};
use serde_json::{Map, Value, json};

/// Pages a catalog may take before the engine is treated as not answering.
const CATALOG_PAGES: usize = 50;

/// `engine_info` says it speaks nu9ve-engine/1 (engine §4), or the error to report.
pub fn check(record: &Record) -> Result<(), nu9ve_json::Error> {
    let info = nu9ve_ipc::request(record, "engine_info", None, None, DEFAULT_TIMEOUT)?;
    if info.get("contract").and_then(Value::as_str) != Some("nu9ve-engine/1") {
        return Err(nu9ve_json::Error::new(
            "incompatible_engine",
            "El motor no habla nu9ve-engine/1.",
        ));
    }
    Ok(())
}

/// The whole catalog of a live engine, its pages merged; valid as a whole, or a failure.
pub fn fetch_catalog(record: &Record, filters: &Map<String, Value>) -> Reply {
    let mut merged: Option<Value> = None;
    let mut cursor: Option<String> = None;
    for _ in 0..CATALOG_PAGES {
        let mut params = Value::Object(filters.clone());
        params["limit"] = json!(500);
        if let Some(c) = &cursor {
            params["cursor"] = json!(c);
        }
        let page =
            match nu9ve_ipc::request(record, "data_catalog", Some(params), None, DEFAULT_TIMEOUT) {
                Ok(page) => page,
                Err(e) => return Reply::Failed(e.code),
            };
        if let Some(problem) = catalog::catalog_problem(&record.app, &page) {
            return Reply::Failed(problem.into());
        }
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        match &mut merged {
            None => merged = Some(page),
            Some(all) => {
                for key in ["roots", "datasets"] {
                    let more = page[key].as_array().cloned().unwrap_or_default();
                    if let Some(list) = all[key].as_array_mut() {
                        for item in more {
                            if !list.contains(&item) {
                                list.push(item);
                            }
                        }
                    }
                }
            }
        }
        if cursor.is_none() {
            break;
        }
    }
    match merged {
        Some(mut all) => {
            all["next_cursor"] = Value::Null;
            // Pages merged: valid as a whole, or not at all.
            match catalog::catalog_problem(&record.app, &all) {
                Some(problem) => Reply::Failed(problem.into()),
                None => Reply::Catalog(all),
            }
        }
        None => Reply::Failed("no_reply".into()),
    }
}
