//! Instalacja pobranych plików (po weryfikacji przypiętego SHA-256 albo zgodzie TOFU), weryfikacja
//! zainstalowanej pozycji i usuwanie. Operacje blokujące (wywoływane w `spawn_blocking`).

use std::path::Path;

use updater_contract::PackageLimits;

use crate::catalog::{Install, ItemSpec};
use crate::fetch::{part_path, sha256_file};
use crate::store::{Hashes, Receipt, Store};
use crate::unpack;

fn io(path: &Path, e: impl std::fmt::Display) -> String {
    format!("{}: {e}", path.display())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = part_path(path);
    std::fs::write(&tmp, bytes).map_err(|e| io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| io(path, e))
}

fn hash_of<'a>(downloads: &'a Hashes, name: &str) -> Result<&'a str, String> {
    downloads
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("brak hasha pliku „{name}”"))
}

/// Przenosi pobrane pliki do katalogu pozycji (zastępuje istniejące).
fn move_files(spec: &ItemSpec, staging: &Path, target: &Path) -> Result<(), String> {
    for f in &spec.files {
        let from = staging.join(&f.name);
        let to = target.join(&f.name);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
        }
        if to.exists() {
            std::fs::remove_file(&to).map_err(|e| io(&to, e))?;
        }
        std::fs::rename(&from, &to).map_err(|e| io(&to, e))?;
    }
    Ok(())
}

/// Instaluje pozycję z katalogu roboczego (`downloads` = zweryfikowane albo zaakceptowane hashe).
pub fn finish(
    store: &Store,
    spec: &ItemSpec,
    downloads: Hashes,
    trusted: bool,
) -> Result<Receipt, String> {
    let staging = store.staging(&spec.id);
    let target = spec.target(store.paths());
    std::fs::create_dir_all(&target).map_err(|e| io(&target, e))?;
    let mut files = Hashes::new();
    for f in &spec.files {
        files.insert(f.name.clone(), hash_of(&downloads, &f.name)?.to_owned());
    }
    match &spec.install {
        Install::Files => move_files(spec, &staging, &target)?,
        Install::Gguf => {
            move_files(spec, &staging, &target)?;
            for f in &spec.files {
                let sha = hash_of(&downloads, &f.name)?;
                let record = providers_local_impl::hash_path(&target.join(&f.name));
                write_atomic(&record, format!("{sha}\n").as_bytes())?;
            }
        }
        Install::Embed(template) => {
            let mut manifest = (**template).clone();
            manifest.model.sha256 = hash_of(&downloads, &manifest.model.path)?.to_owned();
            manifest.tokenizer.sha256 = hash_of(&downloads, &manifest.tokenizer.path)?.to_owned();
            manifest.validate().map_err(|e| e.to_string())?;
            move_files(spec, &staging, &target)?;
            write_atomic(
                &target.join(lib_embed::MANIFEST_FILE),
                manifest.to_json().as_bytes(),
            )?;
        }
        Install::Speaker { manifest, template } => {
            let first = spec.files.first().ok_or("pozycja bez pliku")?;
            let mut json = template.clone();
            json["sha256"] = hash_of(&downloads, &first.name)?.into();
            move_files(spec, &staging, &target)?;
            let text = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
            write_atomic(&target.join(manifest), text.as_bytes())?;
        }
        Install::Pick(picks) => {
            let archive = staging.join(&spec.files.first().ok_or("pozycja bez pliku")?.name);
            files = unpack::extract_picks(&archive, &target, picks, PackageLimits::default())
                .map_err(|e| e.to_string())?;
        }
        Install::Tree { strip, require } => {
            let archive = staging.join(&spec.files.first().ok_or("pozycja bez pliku")?.name);
            files =
                unpack::extract_tree(&archive, &target, strip, require, PackageLimits::default())
                    .map_err(|e| e.to_string())?;
        }
        Install::Manual(_) => return Err("pozycja instalowana ręcznie".into()),
    }
    let receipt = Receipt {
        id: spec.id.clone(),
        trusted,
        downloads,
        files,
        corrupt: None,
    };
    store.save_receipt(&receipt).map_err(|e| e.to_string())?;
    store.clear_staging(&spec.id).map_err(|e| e.to_string())?;
    Ok(receipt)
}

/// Wynik weryfikacji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Wszystkie pliki zgodne z rekordem albo z hashami przypiętymi.
    Ok(Hashes),
    /// Niezgodny albo brakujący plik.
    Corrupt(String),
    /// Pliki obecne, brak wzorca (instalacja ręczna bez przypiętych hashy) — policzone hashe.
    Unknown(Hashes),
}

fn compare(target: &Path, expected: &Hashes) -> Verdict {
    let mut actual = Hashes::new();
    for (rel, want) in expected {
        let path = target.join(rel);
        match sha256_file(&path) {
            Ok(sha) if sha.eq_ignore_ascii_case(want) => {
                actual.insert(rel.clone(), sha);
            }
            Ok(sha) => return Verdict::Corrupt(format!("{rel}: SHA-256 {sha} ≠ {want}")),
            Err(e) => return Verdict::Corrupt(format!("{rel}: {e}")),
        }
    }
    Verdict::Ok(actual)
}

/// Wzorce bez rekordu: hashe przypięte w katalogu, rekord `providers-local` (`.sha256`), `embed.json`.
fn known_hashes(spec: &ItemSpec, target: &Path) -> Hashes {
    let mut out = Hashes::new();
    match &spec.install {
        Install::Files | Install::Gguf | Install::Speaker { .. } => {
            for f in &spec.files {
                let record = providers_local_impl::hash_path(&target.join(&f.name));
                let recorded = std::fs::read_to_string(record).ok();
                let recorded = recorded
                    .map(|r| r.trim().to_owned())
                    .filter(|r| r.len() == 64);
                if let Some(sha) = f.sha256.clone().or(recorded) {
                    out.insert(f.name.clone(), sha);
                }
            }
        }
        Install::Embed(_) => {
            if let Ok((m, _)) =
                lib_embed::EmbedManifest::load(&target.join(lib_embed::MANIFEST_FILE))
            {
                out.insert(m.model.path, m.model.sha256);
                out.insert(m.tokenizer.path, m.tokenizer.sha256);
            }
        }
        Install::Pick(picks) => {
            for p in picks {
                if let Some(sha) = &p.sha256 {
                    out.insert(p.dest.clone(), sha.clone());
                }
            }
        }
        Install::Tree { .. } | Install::Manual(_) => {}
    }
    out
}

/// Weryfikuje zainstalowaną pozycję (rekord menedżera albo wzorce znane bez niego).
pub fn verify(spec: &ItemSpec, target: &Path, receipt: Option<&Receipt>) -> Verdict {
    if let Some(r) = receipt {
        return compare(target, &r.files);
    }
    if !spec.present(target) {
        return Verdict::Corrupt("brak plików pozycji".into());
    }
    let known = known_hashes(spec, target);
    if !known.is_empty() {
        return compare(target, &known);
    }
    let mut computed = Hashes::new();
    for rel in spec.outputs() {
        match sha256_file(&target.join(&rel)) {
            Ok(sha) => {
                computed.insert(rel, sha);
            }
            Err(e) => return Verdict::Corrupt(format!("{rel}: {e}")),
        }
    }
    Verdict::Unknown(computed)
}

fn remove_file(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io(path, e)),
        _ => Ok(()),
    }
}

/// Usuwa pliki pozycji, jej katalog roboczy i rekord.
pub fn remove(store: &Store, spec: &ItemSpec) -> Result<(), String> {
    let target = spec.target(store.paths());
    match &spec.install {
        Install::Tree { .. } | Install::Embed(_) => {
            if target.exists() {
                std::fs::remove_dir_all(&target).map_err(|e| io(&target, e))?;
            }
        }
        Install::Manual(_) => {
            return Err("pozycja instalowana ręcznie — usuń jej pliki sama".into());
        }
        _ => {
            for rel in spec.outputs() {
                remove_file(&target.join(&rel))?;
                if matches!(spec.install, Install::Gguf) {
                    remove_file(&providers_local_impl::hash_path(&target.join(&rel)))?;
                }
            }
        }
    }
    store.clear_staging(&spec.id).map_err(|e| e.to_string())?;
    store.drop_receipt(&spec.id).map_err(|e| e.to_string())
}
