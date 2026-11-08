//! Testy kontraktu `tools-dev`: walidacja, wykrycie projektu, polecenia, parsery.

use super::*;

fn files(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn manifests_and_args() {
    let ms = manifests();
    assert_eq!(ms.len(), 2);
    for m in &ms {
        m.validate().unwrap();
        assert!(
            check_args(&m.name, &sample_args(&m.name)).is_ok(),
            "{}",
            m.name
        );
    }
    assert!(!ms[0].mutating && ms[1].mutating);
    assert_eq!(ms[1].capabilities, vec!["shell.exec".to_owned()]);
    for bad in [
        "--output=x",
        "HEAD & del",
        "a..b",
        "HEAD^",
        "x|y",
        "\"q\"",
        "",
    ] {
        assert!(safe_token(bad).is_err(), "{bad}");
    }
    assert_eq!(safe_token("HEAD~2").unwrap(), "HEAD~2");
    assert_eq!(
        safe_path("src\\main dir/a.rs").unwrap(),
        "src/main dir/a.rs"
    );
    for bad in ["../x", "C:/Windows", "/etc", "-x", "a%PATH%", "a&b"] {
        assert!(safe_path(bad).is_err(), "{bad}");
    }
    assert!(check_args("dev_test", &serde_json::json!({"timeout_s": 0})).is_err());
    assert!(check_args("dev_test", &serde_json::json!({"filter": "parse::tests"})).is_ok());
}

#[test]
fn detects_projects_and_builds_commands() {
    assert_eq!(
        detect_runner(&files(&["Cargo.toml", "src"]), None),
        Some(Runner::Cargo)
    );
    let pkg = r#"{"scripts": {"test": "vitest run"}}"#;
    assert_eq!(
        detect_runner(&files(&["package.json", "pnpm-lock.yaml"]), Some(pkg)),
        Some(Runner::Pnpm)
    );
    assert_eq!(
        detect_runner(&files(&["package.json"]), Some(pkg)),
        Some(Runner::Npm)
    );
    let none = r#"{"scripts": {"test": "echo \"Error: no test specified\" && exit 1"}}"#;
    assert_eq!(detect_runner(&files(&["package.json"]), Some(none)), None);
    assert_eq!(detect_runner(&files(&["go.mod"]), None), Some(Runner::Go));
    assert_eq!(
        detect_runner(&files(&["App.sln"]), None),
        Some(Runner::Dotnet)
    );
    assert_eq!(
        detect_runner(&files(&["pyproject.toml"]), None),
        Some(Runner::Pytest)
    );
    assert_eq!(detect_runner(&files(&["README.md"]), None), None);
    assert_eq!(
        test_command(Runner::Cargo, Some("parse")),
        "cargo test --color never parse"
    );
    assert_eq!(
        test_command(Runner::Pytest, Some("login")),
        "python -m pytest -q -k login"
    );
    let log = git_command(&GitArgs {
        action: GitAction::Log,
        path: Some("src/a b.rs".into()),
        staged: None,
        limit: Some(500),
        rev: None,
    })
    .unwrap();
    assert!(log.contains("log -n 200") && log.ends_with("-- \"src/a b.rs\""));
    let show = git_command(&GitArgs {
        action: GitAction::Show,
        path: None,
        staged: None,
        limit: None,
        rev: None,
    })
    .unwrap();
    assert!(show.ends_with(" HEAD"));
}

#[test]
fn parses_test_outputs() {
    let cargo = "test a::ok ... ok\ntest a::zle ... FAILED\ntest result: FAILED. 5 passed; 1 failed; 2 ignored; 0 measured\ntest result: ok. 3 passed; 0 failed; 0 ignored";
    let c = test_counts(Runner::Cargo, cargo);
    assert_eq!((c.passed, c.failed, c.skipped), (8, 1, 2));
    assert_eq!(c.failures, vec!["a::zle".to_owned()]);
    let py = "FAILED tests/test_x.py::test_login - AssertionError\n==== 1 failed, 4 passed, 1 skipped in 0.31s ====";
    let p = test_counts(Runner::Pytest, py);
    assert_eq!((p.passed, p.failed, p.skipped), (4, 1, 1));
    assert_eq!(p.failures, vec!["tests/test_x.py::test_login".to_owned()]);
    let vitest = " FAIL  src/a.test.ts > liczy\n Test Files  1 failed | 3 passed (4)\n      Tests  1 failed | 20 passed (21)";
    let v = test_counts(Runner::Npm, vitest);
    assert_eq!((v.passed, v.failed), (20, 1));
    let jest = "Tests:       2 failed, 5 passed, 7 total";
    assert_eq!(test_counts(Runner::Npm, jest).failed, 2);
    let go = "--- FAIL: TestLogin (0.00s)\n--- PASS: TestA (0.00s)\nFAIL";
    let g = test_counts(Runner::Go, go);
    assert_eq!((g.passed, g.failed), (1, 1));
    assert_eq!(g.failures, vec!["TestLogin".to_owned()]);
    let net = "  Failed Proj.Tests.Login [12 ms]\nFailed!  - Failed:     1, Passed:    11, Skipped:     0, Total:    12";
    let d = test_counts(Runner::Dotnet, net);
    assert_eq!((d.passed, d.failed), (11, 1));
    assert_eq!(d.failures, vec!["Proj.Tests.Login".to_owned()]);
    assert!(!test_counts(Runner::Cargo, "error[E0425]: cannot find value").found);
}

#[test]
fn parses_git_status() {
    let s = git_status(
        "## main...origin/main [ahead 2, behind 1]\n M src/lib.rs\n?? nowy plik.txt\nA  b.rs",
    );
    assert_eq!(s.branch.as_deref(), Some("main"));
    assert_eq!((s.ahead, s.behind), (2, 1));
    assert_eq!(s.files.len(), 3);
    assert_eq!(
        s.files[1],
        GitFile {
            status: "??".into(),
            path: "nowy plik.txt".into()
        }
    );
    assert!(!s.clean);
    let clean = git_status("## develop\n");
    assert!(clean.clean && clean.branch.as_deref() == Some("develop"));
}
