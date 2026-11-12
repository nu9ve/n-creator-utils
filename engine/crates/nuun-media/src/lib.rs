//! nüün's media domain in Rust, ported method by method from `media/` (rust-engine.md §7).
//! Reads open the catalog read only. The only write so far is `register_volume`, which changes configuration
//! (the catalog), never media, and takes the same writer lock as Python.

pub mod captures;
pub mod catalog;
pub mod devices;
pub mod integrations;
pub mod layout;
pub mod mission_review;
pub mod missions;
pub mod photo_meta;
pub mod planner;
pub mod projects;
pub mod reads;
pub mod rules;
pub mod scan;
pub mod snapshot;
pub mod storage;
pub mod transfer;
pub mod tree;
pub mod units_map;
pub mod volumes;

#[cfg(test)]
mod mission_tests {
    use crate::photo_meta::sample;
    use crate::volumes::test_backend::setup;
    use serde_json::{Value, json};

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
        let params = json!({"source": s.card.display().to_string(), "archive": "MEDIA", "work": "ESSD", "project": "intro"});
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
