//! Zestawy `evals/F5/` zgadzają się z `MANIFEST.json` (SHA-256) — zmiana zestawu bez nowego
//! manifestu = czerwony test (ACCEPTANCE §1).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use sha2::{Digest, Sha256};

const FILES: [(&str, &[u8]); 5] = [
    (
        "bridge-trigger-cases.json",
        include_bytes!("../../../evals/F5/bridge-trigger-cases.json"),
    ),
    (
        "marshal-rules.json",
        include_bytes!("../../../evals/F5/marshal-rules.json"),
    ),
    (
        "parallel-scenarios.json",
        include_bytes!("../../../evals/F5/parallel-scenarios.json"),
    ),
    (
        "scheduler-scenarios.json",
        include_bytes!("../../../evals/F5/scheduler-scenarios.json"),
    ),
    (
        "steering-cases.json",
        include_bytes!("../../../evals/F5/steering-cases.json"),
    ),
];

#[test]
fn f5_sets_match_manifest() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../evals/F5/MANIFEST.json")).unwrap();
    let listed = manifest["files"].as_object().unwrap();
    assert_eq!(listed.len(), FILES.len());
    for (name, bytes) in FILES {
        let hash: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(listed[name], hash, "{name}");
    }
}
