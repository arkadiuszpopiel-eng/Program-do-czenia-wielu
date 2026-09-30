//! Walidacja `ProcessSpec` i budowa wiersza poleceń Windows (reguły `CommandLineToArgvW`).

use std::path::Path;

use platform_contract::{PlatformError, ProcessSpec};

/// Rozszerzenia uruchamiane przez `cmd.exe` — odrzucane (ucieczki argumentów dla plików wsadowych
/// są niebezpieczne, zob. CVE-2024-24576); polecenia powłoki idą przez `tools-shell`.
const BATCH_EXTENSIONS: [&str; 2] = ["bat", "cmd"];

/// Sprawdza specyfikację przed uruchomieniem (niezależnie od platformy).
pub(crate) fn validate_spec(spec: &ProcessSpec) -> Result<(), PlatformError> {
    let cmd = spec.cmd.as_path();
    if cmd.as_os_str().is_empty() || !cmd.is_absolute() {
        // Bez przeszukiwania PATH: ścieżka programu musi być bezwzględna (ochrona przed podmianą).
        return Err(PlatformError::InvalidPath(spec.cmd.clone()));
    }
    if cmd.to_string_lossy().contains('"') {
        return Err(PlatformError::InvalidPath(spec.cmd.clone()));
    }
    let is_batch = cmd
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| BATCH_EXTENSIONS.iter().any(|b| e.eq_ignore_ascii_case(b)));
    if is_batch {
        return Err(PlatformError::Unsupported(format!(
            "{}: pliki wsadowe uruchamia `tools-shell`, nie ProcessPort",
            cmd.display()
        )));
    }
    if spec.args.iter().any(|a| a.contains('\0')) {
        return Err(PlatformError::InvalidPath(spec.cmd.clone()));
    }
    if !spec.cwd.is_absolute() {
        return Err(PlatformError::InvalidPath(spec.cwd.clone()));
    }
    Ok(())
}

/// Dopisuje argument w cudzysłowach zgodnie z regułami parsowania MSVCRT/`CommandLineToArgvW`.
pub(crate) fn push_quoted(out: &mut String, arg: &str) {
    let needs_quotes = arg.is_empty() || arg.contains([' ', '\t', '\n', '\u{b}', '"']);
    if !needs_quotes {
        out.push_str(arg);
        return;
    }
    out.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            other => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(other);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
}

/// Pełny wiersz poleceń: program (zawsze w cudzysłowie) + argumenty.
pub(crate) fn command_line(program: &Path, args: &[String]) -> String {
    let mut out = String::new();
    out.push('"');
    out.push_str(&program.to_string_lossy());
    out.push('"');
    for arg in args {
        out.push(' ');
        push_quoted(&mut out, arg);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn quoted(arg: &str) -> String {
        let mut s = String::new();
        push_quoted(&mut s, arg);
        s
    }

    #[test]
    fn quoting_follows_msvcrt_rules() {
        assert_eq!(quoted("plain"), "plain");
        assert_eq!(quoted(""), "\"\"");
        assert_eq!(quoted("a b"), "\"a b\"");
        assert_eq!(quoted(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(quoted(r"C:\dir with space\"), r#""C:\dir with space\\""#);
        assert_eq!(quoted(r#"a\"b"#), r#""a\\\"b""#);
        assert_eq!(quoted(r"no\quotes\needed"), r"no\quotes\needed");
        assert_eq!(
            command_line(
                Path::new(r"C:\Program Files\w\whisper.exe"),
                &["-m".into(), "model q5.bin".into()]
            ),
            r#""C:\Program Files\w\whisper.exe" -m "model q5.bin""#
        );
    }

    fn spec(cmd: &str) -> ProcessSpec {
        let root = if cfg!(windows) { r"C:\" } else { "/" };
        ProcessSpec {
            cmd: PathBuf::from(format!("{root}{cmd}")),
            args: vec![],
            cwd: PathBuf::from(root),
            integrity: Default::default(),
            memory_limit_mb: None,
        }
    }

    #[test]
    fn spec_validation() {
        assert!(validate_spec(&spec("bin/tool.exe")).is_ok());
        assert!(matches!(
            validate_spec(&spec("x/run.BAT")),
            Err(PlatformError::Unsupported(_))
        ));
        assert!(matches!(
            validate_spec(&spec("x/run.cmd")),
            Err(PlatformError::Unsupported(_))
        ));
        let relative = ProcessSpec {
            cmd: PathBuf::from("tool.exe"),
            ..spec("x")
        };
        assert!(matches!(
            validate_spec(&relative),
            Err(PlatformError::InvalidPath(_))
        ));
        let nul = ProcessSpec {
            args: vec!["a\0b".into()],
            ..spec("t.exe")
        };
        assert!(validate_spec(&nul).is_err());
        let quote = spec("we\"ird.exe");
        assert!(validate_spec(&quote).is_err());
        let rel_cwd = ProcessSpec {
            cwd: PathBuf::from("rel"),
            ..spec("t.exe")
        };
        assert!(validate_spec(&rel_cwd).is_err());
    }
}
