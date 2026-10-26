//! Shared helpers for tests: fixture loading and a scenario directory for the fake `tuple`.

use std::path::{Path, PathBuf};

use serde_json::Value;

pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn fake_tuple_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake_tuple/tuple")
}

pub fn raw_idle() -> Value {
    let text = std::fs::read_to_string(fixtures_dir().join("state_raw_idle.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Every line of the recorded follow stream as `(type, data)`.
pub fn follow_events() -> Vec<(String, Value)> {
    let text = std::fs::read_to_string(fixtures_dir().join("follow_raw_solo_call.jsonl")).unwrap();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let event: Value = serde_json::from_str(line).unwrap();
            (event["type"].as_str().unwrap().to_string(), event["data"].clone())
        })
        .collect()
}

/// A scenario directory the fake `tuple` executable replays from.
pub struct FakeTuple {
    pub dir: PathBuf,
    pub log: PathBuf,
}

impl FakeTuple {
    pub fn new() -> FakeTuple {
        let unique = format!("tuple-ui-test-{}-{}", std::process::id(), rand_suffix());
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(fixtures_dir().join("state_raw_idle.json"), dir.join("state_raw.json")).unwrap();
        let first_line = std::fs::read_to_string(fixtures_dir().join("follow_raw_solo_call.jsonl"))
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_string();
        std::fs::write(dir.join("follow.jsonl"), first_line + "\n").unwrap();
        let log = dir.join("tuple.log");
        FakeTuple { dir, log }
    }

    pub fn env(&self) -> Vec<(String, String)> {
        vec![
            ("FAKE_TUPLE_DIR".into(), self.dir.to_string_lossy().into_owned()),
            ("FAKE_TUPLE_LOG".into(), self.log.to_string_lossy().into_owned()),
        ]
    }

    pub fn emit(&self, event_type: &str, data: Value) {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("events.jsonl"))
            .unwrap();
        writeln!(file, "{}", serde_json::json!({"type": event_type, "data": data})).unwrap();
    }

    pub fn fail_with(&self, message: &str) {
        std::fs::write(self.dir.join("error"), message).unwrap();
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

impl Drop for FakeTuple {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}
