//! Odczyt katalogu silnika (instalacja, dokumentacja) i zrzutów w danych Alfy. Katalog silnika
//! jest tylko czytany — żadna funkcja tego modułu nie tworzy w nim plików.

use std::path::{Path, PathBuf};

use serde_json::Value;
use tools_engine_contract::{BRIDGE_EXE, Capture, DocEntry, EngineError, Installation, SERVER_EXE};

/// Największy plik dokumentacji.
const DOC_MAX_BYTES: u64 = 4 * 1024 * 1024;
/// Pliki w katalogu głównym silnika pokazywane jako dokumentacja.
const ROOT_DOCS: [&str; 4] = ["README.md", "AGENTS.md", "PROGRESS.md", "PLAN_PRODUKCJI.md"];

fn io(path: &Path, e: &std::io::Error) -> EngineError {
    EngineError::Io(format!("{}: {e}", path.display()))
}

fn existing(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

/// Wersja z `[workspace.package]` w `Cargo.toml`.
fn workspace_version(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let mut in_package = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[workspace.package]";
        } else if in_package && line.starts_with("version") {
            let value = line.split_once('=')?.1.trim().trim_matches('"');
            return Some(value.to_owned());
        }
    }
    None
}

/// Stan instalacji w katalogu.
pub(crate) fn installation(root: Option<&Path>) -> Installation {
    let Some(root) = root else {
        return Installation::default();
    };
    let verification: Option<Value> =
        std::fs::read_to_string(root.join("artifacts").join("verification.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());
    let field = |k: &str| {
        verification
            .as_ref()
            .and_then(|v| v.get(k))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    Installation {
        root: Some(root.to_path_buf()),
        server: existing(root.join(SERVER_EXE)),
        bridge: existing(root.join(BRIDGE_EXE)),
        version: workspace_version(root),
        verification: field("status"),
        verified_utc: field("verified_utc"),
    }
}

fn title(path: &Path) -> String {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.starts_with('#'))
                .map(|l| l.trim_start_matches('#').trim().to_owned())
        })
        .unwrap_or_default()
}

/// Lista dokumentacji: pliki główne, `SPEC*.md` i `docs/*.md` (bez podkatalogów).
pub(crate) fn docs(root: &Path) -> Result<Vec<DocEntry>, EngineError> {
    let mut out = Vec::new();
    let mut push = |name: String, path: &Path| {
        if let Ok(meta) = std::fs::metadata(path)
            && meta.is_file()
            && meta.len() <= DOC_MAX_BYTES
        {
            out.push(DocEntry {
                name,
                bytes: meta.len(),
                title: title(path),
            });
        }
    };
    for name in ROOT_DOCS {
        push(name.to_owned(), &root.join(name));
    }
    let mut specs: Vec<_> = std::fs::read_dir(root)
        .map_err(|e| io(root, &e))?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("SPEC") && n.ends_with(".md"))
        .collect();
    specs.sort();
    for name in specs {
        push(name.clone(), &root.join(&name));
    }
    let dir = root.join("docs");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut names: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".md"))
            .collect();
        names.sort();
        for name in names {
            push(format!("docs/{name}"), &dir.join(&name));
        }
    }
    Ok(out)
}

/// Treść pliku z listy — nazwa musi dokładnie zgadzać się z wpisem (bez `..` i ścieżek spoza).
pub(crate) fn read_doc(root: &Path, name: &str) -> Result<String, EngineError> {
    let listed = docs(root)?;
    let entry = listed
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(name.replace('\\', "/").trim()))
        .ok_or_else(|| {
            EngineError::Io(format!(
                "nie ma pliku „{name}” na liście dokumentacji silnika"
            ))
        })?;
    let path = root.join(&entry.name);
    let bytes = std::fs::read(&path).map_err(|e| io(&path, &e))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn telemetry(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

/// Najnowszy zrzut `revision-N.png` (z `revision-N.json`) w katalogu zrzutów okna.
pub(crate) fn latest_capture(dir: &Path) -> Result<Option<Capture>, EngineError> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(None);
    };
    let best = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let rev = name.strip_prefix("revision-")?.strip_suffix(".png")?;
            rev.parse::<u64>().ok()
        })
        .max();
    Ok(best.map(|rev| Capture {
        png: dir.join(format!("revision-{rev}.png")),
        revision: Some(rev),
        telemetry: telemetry(&dir.join(format!("revision-{rev}.json"))),
    }))
}

/// Wynik podglądu (`preview.png` + `preview.json`).
pub(crate) fn preview_capture(dir: &Path) -> Result<Capture, EngineError> {
    let png = dir.join("preview.png");
    if !png.is_file() {
        return Err(EngineError::Io("silnik nie zapisał preview.png".into()));
    }
    let telemetry = telemetry(&dir.join("preview.json"));
    let revision = telemetry.get("revision").and_then(Value::as_u64);
    Ok(Capture {
        png,
        revision,
        telemetry,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine_dir() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers=[]\n[workspace.package]\nedition = \"2024\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(root.join("README.md"), "# NextGO.Gray\ntekst").unwrap();
        std::fs::write(root.join("SPEC_ETAP_2.md"), "# Etap 2").unwrap();
        std::fs::write(root.join("notatki.txt"), "nie").unwrap();
        std::fs::create_dir_all(root.join("docs/stare")).unwrap();
        std::fs::write(root.join("docs/API_ETAP_4.md"), "wstęp\n## API").unwrap();
        std::fs::write(root.join("docs/stare/x.md"), "# ukryte").unwrap();
        std::fs::create_dir_all(root.join("artifacts")).unwrap();
        std::fs::write(
            root.join("artifacts/verification.json"),
            r#"{"status":"passed","verified_utc":"2026-10-26T18:02:36Z"}"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("target/release")).unwrap();
        std::fs::write(root.join(SERVER_EXE), b"MZ").unwrap();
        d
    }

    #[test]
    fn installation_reads_version_binaries_and_verification() {
        let d = engine_dir();
        let i = installation(Some(d.path()));
        assert_eq!(i.version.as_deref(), Some("0.1.0"));
        assert!(i.server.is_some() && i.bridge.is_none());
        assert_eq!(i.verification.as_deref(), Some("passed"));
        assert_eq!(installation(None), Installation::default());
    }

    #[test]
    fn docs_list_and_read_only_listed_files() {
        let d = engine_dir();
        let list = docs(d.path()).unwrap();
        let names: Vec<_> = list.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["README.md", "SPEC_ETAP_2.md", "docs/API_ETAP_4.md"]);
        assert_eq!(list[0].title, "NextGO.Gray");
        assert_eq!(list[2].title, "API");
        assert!(
            read_doc(d.path(), "docs\\API_ETAP_4.md")
                .unwrap()
                .contains("wstęp")
        );
        for bad in ["../x.md", "notatki.txt", "docs/stare/x.md", "Cargo.toml"] {
            assert!(read_doc(d.path(), bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn captures_pick_the_newest_revision() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(latest_capture(&d.path().join("brak")).unwrap(), None);
        for rev in [2, 10, 9] {
            std::fs::write(d.path().join(format!("revision-{rev}.png")), b"png").unwrap();
        }
        std::fs::write(d.path().join("revision-10.json"), r#"{"triangles":12}"#).unwrap();
        std::fs::write(d.path().join("revision-11.png.tmp"), b"x").unwrap();
        let c = latest_capture(d.path()).unwrap().unwrap();
        assert_eq!(c.revision, Some(10));
        assert_eq!(c.telemetry["triangles"], 12);
        assert!(preview_capture(d.path()).is_err());
        std::fs::write(d.path().join("preview.png"), b"png").unwrap();
        std::fs::write(d.path().join("preview.json"), r#"{"revision":4}"#).unwrap();
        assert_eq!(preview_capture(d.path()).unwrap().revision, Some(4));
    }
}
