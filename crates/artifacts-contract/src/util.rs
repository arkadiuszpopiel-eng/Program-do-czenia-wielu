//! Funkcje wspólne dla `-impl` i `-fake`: fakty o pliku (SHA-256, MIME, migawka), wykrycie
//! binarnych, podgląd, diff, katalog wyjściowy, walidacja akcji.

use std::io::Read;
use std::path::{Path, PathBuf};

use sessions_contract::SessionId;
use sha2::{Digest, Sha256};

use crate::types::{
    ArtifactAction, ArtifactError, ArtifactVersion, DiffLine, DiffTag, Preview, TextDiff,
};

/// Ile pierwszych bajtów sprawdzamy przy wykrywaniu plików binarnych.
pub const BINARY_SNIFF_BYTES: usize = 8 * 1024;
/// Domyślny rozmiar podglądu tekstu (64 KiB).
pub const DEFAULT_PREVIEW_BYTES: usize = 64 * 1024;
/// Domyślny limit migawki treści wersji w bazie sesji (1 MiB).
pub const DEFAULT_SNAPSHOT_MAX_BYTES: u64 = 1024 * 1024;

/// Fakty o pliku w chwili rejestracji wersji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    /// Nazwa pliku.
    pub name: String,
    /// Rozmiar (faktycznie przeczytane bajty).
    pub bytes: u64,
    /// SHA-256 (hex).
    pub sha256: String,
    /// MIME z rozszerzenia.
    pub mime: String,
    /// Treść, jeśli rozmiar ≤ limit migawki.
    pub snapshot: Option<Vec<u8>>,
}

/// Czyta plik strumieniowo: SHA-256, rozmiar, MIME, migawka (≤ `snapshot_max` bajtów).
pub fn read_file_facts(path: &Path, snapshot_max: u64) -> Result<FileFacts, ArtifactError> {
    let not_found = || ArtifactError::FileNotFound {
        path: path.to_path_buf(),
    };
    let meta = std::fs::metadata(path).map_err(|_| not_found())?;
    if !meta.is_file() {
        return Err(not_found());
    }
    let mut file = std::fs::File::open(path).map_err(|_| not_found())?;
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    let mut keep: Option<Vec<u8>> = Some(Vec::new());
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(ArtifactError::storage)?;
        if n == 0 {
            break;
        }
        let chunk = &buf[..n];
        hasher.update(chunk);
        total += n as u64;
        keep = keep.filter(|_| total <= snapshot_max).map(|mut k| {
            k.extend_from_slice(chunk);
            k
        });
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(FileFacts {
        name,
        bytes: total,
        sha256: hex(&hasher.finalize()),
        mime: guess_mime(path).to_owned(),
        snapshot: keep,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 (hex, małe litery).
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// MIME z rozszerzenia (małe/duże litery bez znaczenia); nieznane → `application/octet-stream`.
pub fn guess_mime(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "txt" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "ts" => "text/x-typescript",
        "rs" => "text/x-rust",
        "py" => "text/x-python",
        "toml" => "application/toml",
        "json" => "application/json",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/yaml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
}

/// Plik binarny: bajt NUL albo niepoprawny UTF-8 w pierwszych [`BINARY_SNIFF_BYTES`] bajtach
/// (niepełny znak ucięty na końcu próbki nie liczy się jako błąd).
pub fn looks_binary(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(BINARY_SNIFF_BYTES)];
    if sample.contains(&0) {
        return true;
    }
    match std::str::from_utf8(sample) {
        Ok(_) => false,
        Err(e) => e.error_len().is_some(),
    }
}

/// Podgląd treści: binarne → [`Preview::Binary`]; tekst → pierwsze `max_bytes` bajtów uciętych
/// na granicy znaku (`total` = pełny rozmiar wersji).
pub fn preview_bytes(content: &[u8], total: u64, max_bytes: usize, mime: &str) -> Preview {
    if looks_binary(content) {
        return Preview::Binary {
            mime: mime.to_owned(),
            bytes: total,
        };
    }
    let cut = &content[..content.len().min(max_bytes)];
    let valid = match std::str::from_utf8(cut) {
        Ok(_) => cut.len(),
        Err(e) => e.valid_up_to(),
    };
    Preview::Text {
        text: String::from_utf8_lossy(&cut[..valid]).into_owned(),
        truncated: (valid as u64) < total,
    }
}

/// Diff linii (`similar`), numery linii od 1, format ujednolicony z 3 liniami kontekstu.
pub fn diff_texts(old: &str, new: &str) -> TextDiff {
    let diff = similar::TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    let (mut added, mut removed) = (0, 0);
    for change in diff.iter_all_changes() {
        let tag = match change.tag() {
            similar::ChangeTag::Equal => DiffTag::Equal,
            similar::ChangeTag::Insert => {
                added += 1;
                DiffTag::Insert
            }
            similar::ChangeTag::Delete => {
                removed += 1;
                DiffTag::Delete
            }
        };
        lines.push(DiffLine {
            tag,
            old_line: change.old_index().map(|i| i + 1),
            new_line: change.new_index().map(|i| i + 1),
            text: change.value().trim_end_matches(['\n', '\r']).to_owned(),
        });
    }
    let unified = diff
        .unified_diff()
        .context_radius(3)
        .header("a", "b")
        .to_string();
    TextDiff {
        lines,
        added,
        removed,
        unified,
    }
}

/// Katalog wyjściowy sesji: `<root>\Sesje\<dir_name>\out`.
pub fn default_out_dir(root: &Path, dir_name: &str) -> PathBuf {
    root.join("Sesje").join(dir_name).join("out")
}

/// Walidacja akcji: ścieżka docelowa niepusta; przekazanie tylko do innej sesji.
pub fn validate_action(session: &SessionId, action: &ArtifactAction) -> Result<(), ArtifactError> {
    let invalid = |reason: &str| ArtifactError::Invalid {
        reason: reason.to_owned(),
    };
    match action {
        ArtifactAction::SaveAs { target } | ArtifactAction::Zip { target }
            if target.as_os_str().is_empty() =>
        {
            Err(invalid("pusta ścieżka docelowa"))
        }
        ArtifactAction::SendToSession { target } if target == session => {
            Err(invalid("przekazanie do tej samej sesji"))
        }
        _ => Ok(()),
    }
}

/// Numer następnej wersji albo `None`, gdy treść jest identyczna z najnowszą (deduplikacja).
pub fn next_version(existing: &[ArtifactVersion], sha256: &str) -> Option<u32> {
    match existing.last() {
        Some(last) if last.sha256 == sha256 => None,
        Some(last) => Some(last.version + 1),
        None => Some(1),
    }
}

/// Treść wersji: migawka z bazy; bez migawki tylko najnowsza wersja z pliku (co najwyżej `limit`
/// bajtów), jeśli rozmiar pliku się nie zmienił; inaczej [`ArtifactError::ContentUnavailable`].
pub fn version_content(
    version: &ArtifactVersion,
    snapshot: Option<Vec<u8>>,
    latest: bool,
    limit: Option<usize>,
) -> Result<Vec<u8>, ArtifactError> {
    if let Some(bytes) = snapshot {
        return Ok(bytes);
    }
    let unavailable = |reason: &str| ArtifactError::ContentUnavailable {
        reason: reason.to_owned(),
    };
    if !latest {
        return Err(unavailable(
            "brak migawki starszej wersji (plik większy niż limit)",
        ));
    }
    let meta = std::fs::metadata(&version.path).map_err(|_| unavailable("plik usunięty"))?;
    if meta.len() != version.bytes {
        return Err(unavailable("plik zmieniony od rejestracji"));
    }
    let file = std::fs::File::open(&version.path).map_err(|_| unavailable("plik niedostępny"))?;
    let mut out = Vec::new();
    let cap = limit.map_or(u64::MAX, |l| l as u64);
    file.take(cap)
        .read_to_end(&mut out)
        .map_err(ArtifactError::storage)?;
    Ok(out)
}

/// Diff treści dwóch wersji; binarne → [`ArtifactError::NotText`].
pub fn diff_contents(old: &[u8], new: &[u8]) -> Result<TextDiff, ArtifactError> {
    if looks_binary(old) || looks_binary(new) {
        return Err(ArtifactError::NotText);
    }
    Ok(diff_texts(
        &String::from_utf8_lossy(old),
        &String::from_utf8_lossy(new),
    ))
}

#[cfg(test)]
#[path = "util_tests.rs"]
mod tests;
