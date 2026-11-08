//! Wspólne pomocniki testów: zapis zestawu na dysk.

#![allow(dead_code, clippy::unwrap_used)]

use std::path::Path;

use evals_contract::contract_tests::SuiteFixture;

/// Zapisuje pliki zestawu i manifest `<id>.suite.json` w korzeniu.
pub fn write_fixture(root: &Path, fixture: &SuiteFixture) {
    for (path, bytes) in &fixture.files {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, bytes).unwrap();
    }
    let name = format!("{}.suite.json", fixture.manifest.suite);
    let json = serde_json::to_vec_pretty(&fixture.manifest).unwrap();
    std::fs::write(root.join(name), json).unwrap();
}
