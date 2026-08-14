use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs::File, io::BufReader, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayCase {
    pub schema_version: u32,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default)]
    pub inputs: Vec<Value>,
    pub expected: TestOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual: Option<TestOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TestOutcome {
    Return { value: Value },
    Error { class: String, message: String },
    Panic { message: String },
    State { value: Value },
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffRunReport {
    pub schema_version: u32,
    pub replay_path: String,
    pub target: String,
    pub seed: Option<u64>,
    pub status: DiffStatus,
    pub comparisons: Vec<ComparisonReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffStatus {
    Passed,
    Failed,
    Pending,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonReport {
    pub field: &'static str,
    pub status: DiffStatus,
    pub expected: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Value>,
}

pub fn run_replay(path: &Path) -> Result<DiffRunReport> {
    let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let replay: ReplayCase = serde_json::from_reader(BufReader::new(file))
        .with_context(|| format!("cannot parse {}", path.display()))?;
    let mut comparisons = Vec::new();
    let status = match replay.actual.as_ref() {
        Some(actual) if actual == &replay.expected => {
            comparisons.push(ComparisonReport {
                field: "outcome",
                status: DiffStatus::Passed,
                expected: serde_json::to_value(&replay.expected)?,
                actual: Some(serde_json::to_value(actual)?),
            });
            DiffStatus::Passed
        }
        Some(actual) => {
            comparisons.push(ComparisonReport {
                field: "outcome",
                status: DiffStatus::Failed,
                expected: serde_json::to_value(&replay.expected)?,
                actual: Some(serde_json::to_value(actual)?),
            });
            DiffStatus::Failed
        }
        None => {
            comparisons.push(ComparisonReport {
                field: "outcome",
                status: DiffStatus::Pending,
                expected: serde_json::to_value(&replay.expected)?,
                actual: None,
            });
            DiffStatus::Pending
        }
    };

    Ok(DiffRunReport {
        schema_version: 1,
        replay_path: path.to_string_lossy().into_owned(),
        target: replay.target,
        seed: replay.seed,
        status,
        comparisons,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        fs,
        path::PathBuf,
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn compares_matching_replay_outcomes() {
        let path = unique_replay_path("matching");
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "schema_version": 1,
                "target": "net/minecraft/util/Identifier.parse(Ljava/lang/String;)Lnet/minecraft/util/Identifier;",
                "seed": 7,
                "inputs": ["minecraft:stone"],
                "expected": { "kind": "return", "value": "minecraft:stone" },
                "actual": { "kind": "return", "value": "minecraft:stone" }
            }))
            .expect("replay should serialize"),
        )
        .expect("replay should be written");

        let report = run_replay(&path).expect("replay should run");
        assert_eq!(report.status, DiffStatus::Passed);
        assert_eq!(report.comparisons.len(), 1);
    }

    #[test]
    fn marks_missing_actual_outcome_pending() {
        let path = unique_replay_path("pending");
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({
                "schema_version": 1,
                "target": "Direction.byId(I)LDirection;",
                "inputs": [1],
                "expected": { "kind": "return", "value": "east" }
            }))
            .expect("replay should serialize"),
        )
        .expect("replay should be written");

        let report = run_replay(&path).expect("replay should run");
        assert_eq!(report.status, DiffStatus::Pending);
        assert!(report.comparisons[0].actual.is_none());
    }

    fn unique_replay_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/rom-diff-test-{}-{label}-{nanos}.json",
            process::id()
        ))
    }
}
