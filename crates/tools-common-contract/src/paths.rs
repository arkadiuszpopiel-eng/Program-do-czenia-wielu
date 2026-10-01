//! Ścieżki od modelu: rozwinięcie zmiennych (`~`, `%VAR%`, `$env:VAR`), dołączenie katalogu
//! roboczego i twarde odrzucenie postaci, w których semantyka Windows i zakres tokenu mogłyby
//! się rozjechać (`..`, strumienie ADS, prefiksy urządzeń, końcowe kropki/spacje). Dzięki temu
//! ścieżka sprawdzana przez Brokera jest dokładnie tą, na której działa `FsPort`.

use compliance_contract::PathEnv;
use safety_broker_contract::{PathScope, ScopeError};

/// Błąd ścieżki (komunikat dla modelu).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// Pusta albo ze znakami sterującymi.
    #[error("ścieżka pusta albo zawiera znaki sterujące")]
    Empty,
    /// Segment `..` (wyjście z katalogu) — podaj ścieżkę bez `..`.
    #[error("ścieżka „{0}” zawiera `..` — podaj pełną ścieżkę bez `..`")]
    ParentSegment(String),
    /// Postać niejednoznaczna w Windows (ADS `plik:strumień`, `\\?\`, końcowa kropka/spacja).
    #[error(
        "ścieżka „{0}” ma postać niedozwoloną (strumień ADS, prefiks urządzenia, końcowa kropka lub spacja)"
    )]
    Ambiguous(String),
    /// Nieznana zmienna środowiskowa.
    #[error("nieznana zmienna w ścieżce „{0}”")]
    UnknownVariable(String),
    /// Ścieżka względna bez katalogu roboczego.
    #[error("ścieżka względna „{0}” bez katalogu roboczego — podaj ścieżkę bezwzględną")]
    Relative(String),
    /// Zakres tokenu nie powstał.
    #[error("zakres: {0}")]
    Scope(String),
}

impl From<ScopeError> for PathError {
    fn from(e: ScopeError) -> Self {
        Self::Scope(e.to_string())
    }
}

fn expand(raw: &str, env: &PathEnv) -> Result<String, PathError> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    if let Some(after) = rest.strip_prefix('~')
        && (after.is_empty() || after.starts_with(['/', '\\']))
    {
        let home = env
            .get("USERPROFILE")
            .ok_or_else(|| PathError::UnknownVariable(raw.to_owned()))?;
        out.push_str(home.trim_end_matches(['/', '\\']));
        rest = after;
    }
    while let Some(pos) = rest.find(['%', '$']) {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let (name, used) = if let Some(body) = tail.strip_prefix('%') {
            let end = body
                .find('%')
                .ok_or_else(|| PathError::UnknownVariable(raw.to_owned()))?;
            (&body[..end], end + 2)
        } else if tail
            .get(1..5)
            .is_some_and(|p| p.eq_ignore_ascii_case("env:"))
        {
            let body = &tail[5..];
            let end = body
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(body.len());
            (&body[..end], end + 5)
        } else {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
            continue;
        };
        let value = env
            .get(name)
            .filter(|_| !name.is_empty())
            .ok_or_else(|| PathError::UnknownVariable(raw.to_owned()))?;
        out.push_str(value);
        rest = &tail[used..];
    }
    out.push_str(rest);
    Ok(out)
}

fn is_drive_prefix(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// Czy ścieżka jest bezwzględna (dysk, UNC albo zaczyna się separatorem).
pub fn is_absolute(path: &str) -> bool {
    path.starts_with(['/', '\\'])
        || (path.get(..2).is_some_and(is_drive_prefix)
            && path.get(2..).is_some_and(|r| r.starts_with(['/', '\\'])))
}

/// Rozwija zmienne, dołącza katalog roboczy do ścieżki względnej i sprawdza postać.
pub fn resolve_path(raw: &str, workdir: Option<&str>, env: &PathEnv) -> Result<String, PathError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return Err(PathError::Empty);
    }
    if ["\\\\?\\", "\\\\.\\", "\\??\\", "//?/", "//./"]
        .iter()
        .any(|p| trimmed.starts_with(p))
    {
        return Err(PathError::Ambiguous(trimmed.to_owned()));
    }
    let expanded = expand(trimmed, env)?;
    let full = if is_absolute(&expanded) {
        expanded
    } else {
        let base = workdir.ok_or_else(|| PathError::Relative(trimmed.to_owned()))?;
        let sep = if base.contains('\\') { '\\' } else { '/' };
        format!("{}{sep}{expanded}", base.trim_end_matches(['/', '\\']))
    };
    for (i, seg) in full.split(['/', '\\']).enumerate() {
        if seg == ".." {
            return Err(PathError::ParentSegment(trimmed.to_owned()));
        }
        let drive = i == 0 && is_drive_prefix(seg);
        let ads = seg.contains(':') && !drive;
        let trailing = seg != "." && !seg.is_empty() && seg.ends_with(['.', ' ']);
        if ads || trailing || seg.contains(['*', '?', '"', '<', '>', '|']) {
            return Err(PathError::Ambiguous(trimmed.to_owned()));
        }
    }
    Ok(full)
}

/// Czy którykolwiek segment ścieżki jest na deny-liście poświadczeń platformy
/// (`.ssh`, `.claude`, `.codex`, `Credentials`, `Login Data`, `Cookies`) — niezależnie od
/// separatora (`/` lub `\`) i systemu, na którym działa test.
pub fn has_credential_segment(path: &str) -> bool {
    path.split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .any(|s| platform_contract::is_credential_path(std::path::Path::new(s)))
}

/// Zakres dokładnie jednej ścieżki (do tokenu).
pub fn exact_scope(path: &str, env: &PathEnv) -> Result<PathScope, PathError> {
    Ok(PathScope::exact(path, env)?)
}

/// Zakres poddrzewa (katalog roboczy, listowanie, wyszukiwanie, snapshot).
pub fn tree_scope(path: &str, env: &PathEnv) -> Result<PathScope, PathError> {
    Ok(PathScope::tree(path, env)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> PathEnv {
        PathEnv::windows_profile(r"C:\Users\ala").with("TEMP", r"C:\Users\ala\AppData\Local\Temp")
    }

    #[test]
    fn expands_and_joins() {
        let e = env();
        assert_eq!(
            resolve_path("~/Docs/a.txt", None, &e).unwrap(),
            r"C:\Users\ala/Docs/a.txt"
        );
        assert_eq!(
            resolve_path(r"%USERPROFILE%\x", None, &e).unwrap(),
            r"C:\Users\ala\x"
        );
        assert_eq!(
            resolve_path(r"$env:TEMP\y", None, &e).unwrap(),
            r"C:\Users\ala\AppData\Local\Temp\y"
        );
        assert_eq!(
            resolve_path("a/b.txt", Some("/Users/ala/w/"), &e).unwrap(),
            "/Users/ala/w/a/b.txt"
        );
        assert_eq!(
            resolve_path("b.txt", Some(r"C:\w"), &e).unwrap(),
            r"C:\w\b.txt"
        );
        assert_eq!(
            resolve_path("cost$5.txt", Some("/w"), &e).unwrap(),
            "/w/cost$5.txt"
        );
        assert!(is_absolute(r"C:\x") && is_absolute("/x") && !is_absolute("C:x"));
    }

    #[test]
    fn rejects_dangerous_forms() {
        let e = env();
        for bad in [
            "",
            "a\u{0}b",
            "/Users/ala/../bob/x",
            r"C:\Users\ala\Docs\..\..\bob",
            r"\\?\C:\Windows",
            r"C:\x\file.txt:secret",
            r"C:\x\name. ",
            r"C:\x\dir.\f",
            "%NIEZNANA%\\x",
            "$env:NIEMA\\x",
            r"C:\x\*.txt",
        ] {
            assert!(resolve_path(bad, Some("/w"), &e).is_err(), "{bad:?}");
        }
        assert!(matches!(
            resolve_path("rel.txt", None, &e),
            Err(PathError::Relative(_))
        ));
        assert!(resolve_path("~x", Some("/w"), &e).is_ok());
        assert!(resolve_path("$żółw/x", Some("/w"), &e).is_ok());
    }

    #[test]
    fn credential_segments() {
        assert!(has_credential_segment(r"C:\Users\ala\.ssh\id_rsa"));
        assert!(has_credential_segment("/Users/ala/.claude/x.json"));
        assert!(has_credential_segment(
            r"C:\Users\ala\AppData\Local\Google\Chrome\User Data\Default\Login Data"
        ));
        assert!(!has_credential_segment("/Users/ala/claude-notes.md"));
        let scope = exact_scope("/Users/ala/x", &env()).unwrap();
        assert_eq!(scope.canonical(), r"c:\users\ala\x");
        assert!(tree_scope("rel", &env()).is_err());
    }
}
