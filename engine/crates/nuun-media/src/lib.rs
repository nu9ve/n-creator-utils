//! nüün's media domain in Rust, ported method by method from `media/` (rust-engine.md §7).
//! Read and write operations share the catalog and on-disk protocol with Python.

#[cfg(target_os = "macos")]
mod ableton;
pub mod accumulations;
pub mod action;
pub mod agent_storage;
pub mod captures;
pub mod catalog;
pub mod clipper;
mod clipper_release;
pub mod closeout;
#[cfg(target_os = "macos")]
pub mod collection_release;
pub mod conversations;
pub mod coverage;
pub mod delivery;
pub mod devices;
#[cfg(target_os = "macos")]
pub mod dng;
mod editorial;
pub mod handoff;
mod handycam;
pub mod integrations;
#[cfg(any(target_os = "macos", windows))]
mod interrupt;
pub mod layout;
#[cfg(target_os = "macos")]
pub mod living;
pub mod mission_review;
pub mod missions;
pub mod photo_meta;
pub mod planner;
pub mod project_keys;
pub mod projects;
pub mod reads;
pub mod recovery;
#[cfg(unix)]
pub mod relayout;
pub mod release;
pub mod restore;
#[cfg(unix)]
pub mod retitle;
#[cfg(unix)]
pub mod revisions;
pub mod rules;
pub mod save_all;
pub mod scan;
pub mod settings;
pub mod snapshot;
pub mod sony;
pub mod sources;
pub mod storage;
pub mod suggestions;
pub mod transcripts;
pub mod transfer;
pub mod tree;
pub mod units_map;
pub mod volumes;
pub mod work_copy;

#[cfg(test)]
mod mission_tests {
    use crate::photo_meta::sample;
    use crate::volumes::test_backend::setup;
    use serde_json::{Value, json};

    #[test]
    fn late_mission_refreshes_draft_and_associates_verified_archive_without_moving() {
        let s = setup();
        let dir = s.card.join("DCIM/101MEDIA");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("DJI_0001.JPG"),
            sample::jpeg("DJI", "FC3170", "2026:11:04 16:32:12", None),
        )
        .unwrap();
        std::fs::write(
            dir.join("DJI_0001.DNG"),
            sample::tiff("DJI", "FC3170", "2026:11:04 16:32:12"),
        )
        .unwrap();
        let mut params = json!({"source":s.card, "archive":"MEDIA","work":"EDICION","project":"intro","cleanup":false});
        let pending = crate::planner::build_plan(&s.home, &s.backend, &params).unwrap();
        crate::transfer::apply(
            &s.home,
            &s.backend,
            &pending,
            pending["digest"].as_str().unwrap(),
            &|_, _| Ok(()),
        )
        .unwrap();
        params["project"] = json!("next-project");
        let draft = crate::planner::build_plan(&s.home, &s.backend, &params).unwrap();
        crate::save_all::save_draft(&s.home, &json!({"key":"card","document":draft.to_string()}))
            .unwrap();
        let area = s.archive.join("004_datos/subaereo/coverage");
        std::fs::create_dir_all(&area).unwrap();
        std::fs::write(area.join("history.json"), json!({"plans":[{"id":"P1","name":"Calles"}],
            "captureLog":[{"captureID":"C1","planID":"P1","sessionID":"S1","mediaCreatedAt":"2026-11-04 16:32:13",
                "imageJPEGReference":"DJI_0001.JPG","latitude":29.09,"longitude":-110.9583333}]}).to_string()).unwrap();
        let update =
            crate::save_all::replan_dji(&s.home, &s.backend, &json!({"key":"card"})).unwrap();
        assert_eq!(update["before"]["subaereo"]["units"]["pending_export"], 1);
        assert_eq!(update["after"]["subaereo"]["units"]["matched"], 1);
        assert_eq!(
            crate::save_all::draft(&s.home).unwrap()["plans"]["card"],
            draft.to_string()
        );
        let catalog = crate::catalog::Catalog::open(&s.home).unwrap();
        let options = json!({"archive":"MEDIA"});
        let review = crate::missions::archived_review(&catalog, &s.backend, &options).unwrap();
        assert_eq!(review["matched"], 1);
        let path = review["sample"][0]["paths"][0].as_str().unwrap();
        assert!(s.archive.join(path).is_file());
        let applied = crate::missions::archived_apply(
            &s.home,
            &s.backend,
            &json!({"archive":"MEDIA","digest":review["digest"]}),
        )
        .unwrap();
        assert_eq!(applied["associated"], 1);
        assert_eq!(applied["files_moved"], 0);
        assert!(s.archive.join(path).is_file());
        assert_eq!(
            crate::missions::archived_review(&catalog, &s.backend, &options).unwrap()["recorded"],
            1
        );
    }

    /// tests/test_mission_match.py: a matched drone still goes to the mission folders and into units.ndjson.
    #[test]
    fn a_matched_mission_lands_in_its_folders_and_in_the_units_map() {
        let s = setup();
        let dir = s.card.join("DCIM/101MEDIA");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("DJI_0001.JPG"),
            sample::jpeg("DJI", "FC3170", "2026:11:04 16:32:12", None),
        )
        .unwrap();
        std::fs::write(
            dir.join("DJI_0001.DNG"),
            sample::tiff("DJI", "FC3170", "2026:11:04 16:32:12"),
        )
        .unwrap();
        let area = s.archive.join("004_datos/subaereo/coverage");
        std::fs::create_dir_all(&area).unwrap();
        let history = json!({"plans": [{"id": "P1", "name": "Calles-45m-1104-1625"}], "captureLog": [
            {"captureID": "C1", "planID": "P1", "sessionID": "S1", "mediaCreatedAt": "2026-11-04 16:32:13",
             "imageJPEGReference": "DJI_0001.JPG", "latitude": 29.09, "longitude": -110.9583333}]});
        std::fs::write(area.join("history.json"), history.to_string()).unwrap();
        let params = json!({"source": s.card.display().to_string(), "archive": "MEDIA", "work": "EDICION", "project": "intro"});
        let plan = crate::planner::build_plan(&s.home, &s.backend, &params).unwrap();
        let digest = plan["digest"].as_str().unwrap();
        let result =
            super::transfer::apply(&s.home, &s.backend, &plan, digest, &|_: &str, _: &Value| {
                Ok(())
            })
            .unwrap();
        assert_eq!(result["state"], "source_retained");
        assert_eq!(result["units_map"], "written");
        // The session starts at its first registry record (16:32:13, camera time).
        let session = "2026/11/26NOV04-163213_CALLES-45M-1104-1625";
        let work = s
            .work
            .join("001_proyectos/atlas-subaereo/misiones")
            .join(session);
        assert!(
            work.join("26NOV04-163212_MAVICAIR2_MX-SUBAEREO[].JPG")
                .is_file(),
            "{}",
            work.display()
        );
        assert!(
            !work
                .join("26NOV04-163212_MAVICAIR2_MX-SUBAEREO[].DNG")
                .exists(),
            "DNG stays archive-only"
        );
        let map = std::fs::read_to_string(
            s.work
                .join("001_proyectos/atlas-subaereo/_nuun/units.ndjson"),
        )
        .unwrap();
        let line: Value = serde_json::from_str(map.lines().next().unwrap()).unwrap();
        assert_eq!(
            line["mission"],
            json!({"planID": "P1", "sessionID": "S1", "captureID": "C1"})
        );
        assert_eq!(
            (line["original_name"].as_str(), line["dcim_folder"].as_str()),
            (Some("DJI_0001.JPG"), Some("101MEDIA"))
        );
        assert_eq!(
            line["captured_at_utc"], "2026-11-04T23:32:12Z",
            "the EXIF offset -07:00"
        );
        assert_eq!(line["utc_basis"], "exif_offset");
        assert_eq!(
            line["work"]["jpg"],
            format!("misiones/{session}/26NOV04-163212_MAVICAIR2_MX-SUBAEREO[].JPG")
        );
        assert!(line["work"]["dng"].is_null());
        assert_eq!(line["sha256"]["dng"].as_str().unwrap().len(), 64);
    }
}

pub mod suite_device;
pub mod suite_profiles;
pub mod suite_volume;
