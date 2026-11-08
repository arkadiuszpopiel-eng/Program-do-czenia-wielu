//! Sprawdzenie pliku przed przyjęciem jako załącznik: ścieżka bezwzględna, zwykły plik, ani ona,
//! ani cel jej dowiązań nie leży w danych Alfy, na deny-liście (`compliance`) ani w katalogu
//! poświadczeń (`~/.claude`, `~/.codex`, profile przeglądarek…) — AGENTS.md: kod Alfy nigdy ich
//! nie czyta. Nazwa kopii: bez separatorów, znaków sterujących i nazw zarezerwowanych Windows.

use std::path::{Path, PathBuf};

use app_api::dto::AttachmentRejectReason;
use app_api::paths::AppPaths;
use compliance_contract::DenyChecker;
use compliance_contract::deny::normalize;
use tools_common_contract::paths;

/// Strażnik ścieżek załączników (deny-listy jak dla narzędzi agentek + katalogi danych Alfy).
pub struct PathGuard {
    deny: DenyChecker,
    env: compliance_contract::PathEnv,
    internal: Vec<compliance_contract::deny::NormPath>,
}

impl PathGuard {
    /// Strażnik dla katalogów aplikacji.
    pub fn new(app: &AppPaths) -> Self {
        let (_, env) = app_modules::broker::path_env_for(&app.user_root);
        let internal = app_modules::workdir::protected_dirs(app)
            .iter()
            .map(|d| normalize(d, &env))
            .collect();
        Self {
            deny: DenyChecker::new(app_modules::workdir::tool_deny_lists(app), &env),
            env,
            internal,
        }
    }

    fn hit(&self, path: &str) -> bool {
        let norm = normalize(path, &self.env);
        self.internal.iter().any(|dir| within(&norm, dir))
            || self.deny.is_denied_normalized(&norm)
            || paths::has_credential_segment(path)
    }

    /// Sprawdza plik źródłowy; `Ok(rozmiar)` albo powód odrzucenia.
    pub fn check(&self, path: &Path) -> Result<u64, AttachmentRejectReason> {
        if !path.is_absolute() {
            return Err(AttachmentRejectReason::NotAFile);
        }
        let text = path.to_string_lossy().into_owned();
        if paths::protected_with_links(&text, |p| self.hit(p)) {
            return Err(AttachmentRejectReason::Denied);
        }
        let meta = std::fs::metadata(path).map_err(|_| AttachmentRejectReason::Unreadable)?;
        if !meta.is_file() {
            return Err(AttachmentRejectReason::NotAFile);
        }
        Ok(meta.len())
    }
}

impl PathGuard {
    /// Sprawdza katalog (np. kopii zapasowych): bezwzględny, istniejący, a ani on, ani cel jego
    /// dowiązań nie leży w danych Alfy, na deny-liście ani w katalogu poświadczeń.
    pub fn check_dir(&self, path: &Path) -> Result<(), AttachmentRejectReason> {
        if !path.is_absolute() || !path.is_dir() {
            return Err(AttachmentRejectReason::NotAFile);
        }
        let text = path.to_string_lossy().into_owned();
        if paths::protected_with_links(&text, |p| self.hit(p)) {
            return Err(AttachmentRejectReason::Denied);
        }
        Ok(())
    }
}

/// Czy `a` leży w `b` (komponenty w normalizacji Windows).
fn within(
    a: &compliance_contract::deny::NormPath,
    b: &compliance_contract::deny::NormPath,
) -> bool {
    a.root == b.root
        && b.comps.len() <= a.comps.len()
        && b.comps
            .iter()
            .zip(&a.comps)
            .all(|(p, c)| compliance_contract::deny::comp_matches(c, p))
}

const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Bezpieczna nazwa pliku (≤ 120 znaków, bez `\ / : * ? " < > |`, znaków sterujących, kropek
/// i spacji na końcu oraz nazw zarezerwowanych Windows); pusta → `zalacznik`.
pub fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .take(120)
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let stem = trimmed.split('.').next().unwrap_or_default().to_lowercase();
    if trimmed.is_empty() || RESERVED.contains(&stem.as_str()) {
        format!("zalacznik{}", if trimmed.is_empty() { "" } else { "-" })
            + trimmed.trim_start_matches(|c: char| c != '.')
    } else {
        trimmed.to_owned()
    }
}

/// Pierwsza wolna ścieżka `dir/nazwa`, `dir/nazwa (2).ext`, … (bez nadpisywania).
pub fn unique_in(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    (2..10_000)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join(format!("{stem}-{}{ext}", std::process::id())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitized() {
        assert_eq!(safe_name("raport Q3.pdf"), "raport Q3.pdf");
        assert_eq!(safe_name("..\\..\\win.ini"), "_.._win.ini");
        assert_eq!(safe_name("a/b:c*?.txt"), "a_b_c__.txt");
        assert_eq!(safe_name("  ..  "), "zalacznik");
        assert_eq!(safe_name("CON.txt"), "zalacznik-.txt");
        assert_eq!(safe_name("nul"), "zalacznik-");
        assert_eq!(safe_name("x\u{0}y\n.md"), "x_y_.md");
        assert_eq!(safe_name(&"a".repeat(300)).chars().count(), 120);
    }

    #[test]
    fn unique_names_do_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let first = unique_in(dir.path(), "a.txt");
        std::fs::write(&first, "1").unwrap();
        let second = unique_in(dir.path(), "a.txt");
        assert_eq!(second, dir.path().join("a (2).txt"));
        std::fs::write(&second, "2").unwrap();
        assert_eq!(unique_in(dir.path(), "a.txt"), dir.path().join("a (3).txt"));
        assert_eq!(unique_in(dir.path(), "README"), dir.path().join("README"));
    }

    #[test]
    fn alfa_data_and_credentials_are_denied() {
        let root = tempfile::tempdir().unwrap();
        let app = AppPaths::under(root.path());
        std::fs::create_dir_all(&app.local).unwrap();
        std::fs::create_dir_all(app.user_root.join("Dokumenty")).unwrap();
        let guard = PathGuard::new(&app);
        let ok = app.user_root.join("Dokumenty").join("plan.txt");
        std::fs::write(&ok, "plan").unwrap();
        assert_eq!(guard.check(&ok), Ok(4));
        let db = app.local.join("sesja.db");
        std::fs::write(&db, "x").unwrap();
        assert_eq!(guard.check(&db), Err(AttachmentRejectReason::Denied));
        let cred = root.path().join(".claude").join(".credentials.json");
        std::fs::create_dir_all(cred.parent().unwrap()).unwrap();
        std::fs::write(&cred, "{}").unwrap();
        assert_eq!(guard.check(&cred), Err(AttachmentRejectReason::Denied));
        assert_eq!(
            guard.check(Path::new("wzgledna.txt")),
            Err(AttachmentRejectReason::NotAFile)
        );
        assert_eq!(
            guard.check(&app.user_root.join("Dokumenty")),
            Err(AttachmentRejectReason::NotAFile)
        );
        assert_eq!(
            guard.check(&app.user_root.join("brak.txt")),
            Err(AttachmentRejectReason::Unreadable)
        );
        assert_eq!(guard.check_dir(&app.user_root.join("Dokumenty")), Ok(()));
        assert_eq!(
            guard.check_dir(&app.local),
            Err(AttachmentRejectReason::Denied)
        );
        assert_eq!(guard.check_dir(&ok), Err(AttachmentRejectReason::NotAFile));
    }
}
