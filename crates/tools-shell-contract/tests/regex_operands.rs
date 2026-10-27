//! A PowerShell regex operand is data, not a UNC network path.
use tools_shell_contract::{analyze, analyze_powershell};
#[test]
fn obj_bin_alternative_without_group_is_also_literal_data() {
    let a = analyze_powershell(
        r#"Get-ChildItem | Where-Object { $_.FullName -notmatch '\\obj|bin\\' }"#,
    );
    assert!(!a.network, "{a:?}");
    assert!(a.hosts.is_empty() && a.paths.is_empty(), "{a:?}");
}
#[test]
fn invoked_unc_expression_is_not_a_literal_pattern() {
    let a = analyze_powershell(r#"$x -match (& '\\server\share\pattern.ps1')"#);
    assert!(a.network, "{a:?}");
    assert_eq!(a.hosts, ["server"]);
}
#[test]
fn native_program_match_argument_does_not_hide_actual_unc_file() {
    for cmd in [
        r#"findstr.exe -match '\\server\share\file.txt'"#,
        r#"findstr.exe $x -match '\\server\share\file.txt'"#,
    ] {
        let a = analyze_powershell(cmd);
        assert!(a.network, "{a:?}");
        assert_eq!(a.hosts, ["server"]);
    }
}
#[test]
fn literal_single_quote_regex_can_contain_end_anchor() {
    let a = analyze_powershell(
        r#"Get-ChildItem | Where-Object { $_.FullName -notmatch '\\(obj|bin)(\\|$)' }"#,
    );
    assert!(!a.network, "{a:?}");
}
#[test]
fn local_obj_bin_filter_is_not_network_or_external_path() {
    for op in [
        "-match",
        "-notmatch",
        "-imatch",
        "-inotmatch",
        "-cmatch",
        "-cnotmatch",
        "-replace",
        "-ireplace",
        "-creplace",
    ] {
        let command = format!(
            r#"Get-ChildItem -Recurse | Where-Object {{ $_.FullName {op} '\\(obj|bin)\\' }} | Select-Object FullName"#
        );
        let a = analyze_powershell(&command);
        assert!(!a.network, "{command}: {a:?}");
        assert!(a.hosts.is_empty(), "{a:?}");
        assert!(
            a.paths.is_empty(),
            "regex is not a filesystem target: {a:?}"
        );
    }
}
#[test]
fn nested_regex_filter_keeps_local_classification() {
    let a = analyze_powershell(
        r#"powershell -Command "Get-ChildItem | Where-Object { $_.FullName -notmatch '\\(obj|bin)\\' }""#,
    );
    assert!(!a.network, "{a:?}");
    assert!(a.hosts.is_empty(), "{a:?}");
}
#[test]
fn actual_unc_path_after_regex_still_requires_network() {
    let a = analyze_powershell(
        r#"Get-ChildItem | Where-Object { $_.FullName -notmatch '\\(obj|bin)\\' }; Get-Content '\\server\share\a.txt'"#,
    );
    assert!(a.network);
    assert_eq!(a.hosts, ["server"]);
}
#[test]
fn real_upload_after_regex_keeps_actual_host() {
    let a = analyze_powershell(
        r#"Get-ChildItem | Where-Object { $_.FullName -notmatch '\\(obj|bin)\\' }; curl -d @a.txt https://example.net/upload"#,
    );
    assert!(a.network);
    assert_eq!(a.hosts, ["example.net"]);
}
#[test]
fn interpolated_pattern_is_not_assumed_literal() {
    let a = analyze_powershell(r#"$x -match "\\$server\share""#);
    assert!(
        a.network,
        "dynamic data must not erase existing network detection: {a:?}"
    );
}
#[test]
fn cmd_does_not_get_the_regex_exception() {
    // W cmd `;` rozdziela argumenty, a "…" to zwykła ścieżka: `type` otwiera udział SMB.
    for cmd in [
        r#"type ; $x -match "\\evil\share\f.txt""#,
        r#"type { $x -match "\\evil\share\f.txt""#,
    ] {
        let a = analyze(cmd);
        assert!(a.network, "{cmd}: {a:?}");
        assert_eq!(a.hosts, ["evil"], "{cmd}");
    }
}
#[test]
fn cmd_nested_in_powershell_does_not_get_the_regex_exception() {
    let a = analyze_powershell(r#"cmd /c "type ; $x -match '\\evil\share\f.txt'""#);
    assert!(a.network, "{a:?}");
    assert_eq!(a.hosts, ["evil"]);
}
#[test]
fn shell_agnostic_analysis_stays_conservative() {
    let a = analyze(r#"Get-ChildItem | Where-Object { $_.FullName -notmatch '\\(obj|bin)\\' }"#);
    assert!(a.network, "{a:?}");
}
