//! Zamrożenie podziału `test` (protokół nagrań: „hash listy nazw + SHA-256 plików, same pliki poza
//! gitem”). Lista w formacie `sha256sum`: linia na każdy plik audio podziału test
//! (`<sha256>  <ścieżka względna>`) i linia `<sha256>  manifest:test` — skrót kanonicznych pozycji
//! test (posortowanych po `id`, JSON bez spacji, LF). Lista nie zawiera transkrypcji ani audio.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::eval::manifest::{ManifestEntry, Split};

/// Nazwa linii ze skrótem manifestu.
pub const MANIFEST_LINE: &str = "manifest:test";

/// SHA-256 jako 64 znaki hex.
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn test_entries(entries: &[ManifestEntry]) -> Vec<&ManifestEntry> {
    let mut t: Vec<&ManifestEntry> = entries.iter().filter(|e| e.split == Split::Test).collect();
    t.sort_by(|a, b| a.id.cmp(&b.id));
    t
}

/// Skrót kanonicznych pozycji podziału test.
pub fn manifest_digest(entries: &[ManifestEntry]) -> String {
    let canon: String = test_entries(entries)
        .into_iter()
        .filter_map(|e| serde_json::to_string(e).ok())
        .map(|l| l + "\n")
        .collect();
    sha256_hex(canon.as_bytes())
}

fn audio_hashes(entries: &[ManifestEntry], root: &Path) -> (BTreeMap<String, String>, Vec<String>) {
    let mut out = BTreeMap::new();
    let mut errors = Vec::new();
    for e in test_entries(entries) {
        if out.contains_key(&e.audio) {
            continue;
        }
        match std::fs::read(root.join(&e.audio)) {
            Ok(bytes) => {
                out.insert(e.audio.clone(), sha256_hex(&bytes));
            }
            Err(err) => errors.push(format!("{}: {} ({err})", e.id, e.audio)),
        }
    }
    (out, errors)
}

/// Lista zamrożenia (do `evals/acceptance/F2/test.sha256`; zmiana wymaga przeglądu człowieka).
pub fn freeze_list(entries: &[ManifestEntry], root: &Path) -> Result<String, Vec<String>> {
    let (hashes, errors) = audio_hashes(entries, root);
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut s = format!("{}  {MANIFEST_LINE}\n", manifest_digest(entries));
    for (path, hash) in hashes {
        let _ = writeln!(s, "{hash}  {path}");
    }
    Ok(s)
}

/// Sprawdza manifest i pliki względem listy zamrożenia; zwraca wszystkie rozbieżności.
pub fn verify_frozen(entries: &[ManifestEntry], root: &Path, list: &str) -> Vec<String> {
    let mut frozen: BTreeMap<String, String> = BTreeMap::new();
    let mut errors = Vec::new();
    for (i, line) in list
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        match line.split_once("  ") {
            Some((hash, path)) if hash.len() == 64 => {
                frozen.insert(path.to_owned(), hash.to_owned());
            }
            _ => errors.push(format!(
                "lista zamrożenia, linia {}: niepoprawny format",
                i + 1
            )),
        }
    }
    if frozen.remove(MANIFEST_LINE).as_deref() != Some(manifest_digest(entries).as_str()) {
        errors.push("pozycje podziału test różnią się od zamrożonych".into());
    }
    let (hashes, read_errors) = audio_hashes(entries, root);
    errors.extend(read_errors);
    for (path, hash) in &hashes {
        match frozen.remove(path) {
            Some(h) if &h == hash => {}
            Some(_) => errors.push(format!("{path}: plik zmieniony po zamrożeniu")),
            None => errors.push(format!("{path}: plik spoza zamrożonego zestawu")),
        }
    }
    errors.extend(
        frozen
            .keys()
            .map(|p| format!("{p}: brak w manifeście (zestaw zmniejszony)")),
    );
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::manifest::parse_manifest;

    fn manifest() -> Vec<ManifestEntry> {
        let line = |id: &str, split: &str| {
            format!(
                r#"{{"id":"{id}","audio":"16k/{id}.wav","kind":"free_speech","split":"{split}","conditions":{{"machine":"desktop","environment":"quiet","mic":"usb"}},"transcript":"tak"}}"#
            )
        };
        parse_manifest(&[line("a", "test"), line("b", "dev"), line("c", "test")].join("\n"))
            .unwrap()
    }

    #[test]
    fn freeze_and_verify_detect_every_change() {
        let dir = std::env::temp_dir().join(format!("alfa-f2-freeze-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("16k")).unwrap();
        for id in ["a", "b", "c"] {
            std::fs::write(dir.join(format!("16k/{id}.wav")), id.as_bytes()).unwrap();
        }
        let entries = manifest();
        let list = freeze_list(&entries, &dir).unwrap();
        assert_eq!(list.lines().count(), 3, "{list}");
        assert!(!list.contains("16k/b.wav"), "dev nie jest zamrażany");
        assert_eq!(
            sha256_hex(b"a"),
            "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb"
        );
        assert!(verify_frozen(&entries, &dir, &list).is_empty());
        std::fs::write(dir.join("16k/c.wav"), b"zmiana").unwrap();
        let mut changed = entries.clone();
        changed[0].transcript = Some("nie".into());
        let errors = verify_frozen(&changed, &dir, &list);
        assert!(
            errors.iter().any(|e| e.contains("różnią się")),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e == "16k/c.wav: plik zmieniony po zamrożeniu"),
            "{errors:?}"
        );
        let fewer: Vec<_> = entries.iter().filter(|e| e.id != "a").cloned().collect();
        let errors = verify_frozen(&fewer, &dir, &format!("{list}zły wiersz\n"));
        assert!(
            errors
                .iter()
                .any(|e| e == "16k/a.wav: brak w manifeście (zestaw zmniejszony)"),
            "{errors:?}"
        );
        assert!(errors.iter().any(|e| e.contains("linia 4")), "{errors:?}");
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(freeze_list(&entries, &dir).is_err());
    }
}
