//! Kwarantanna pobrań `tools-net` (PLAN §7.2 „Sieć”, §8.7): plik z sieci trafia wyłącznie do
//! katalogu kwarantanny sesji jako **nowy** plik (częściowy `.part` tworzony bez nadpisania,
//! nazwa końcowa bez nadpisania istniejącego), katalog nie może prowadzić przez dowiązanie ani
//! junction (ścieżka rzeczywista = podana), nazwa od serwera jest oczyszczana, a na Windows plik
//! dostaje znacznik Mark-of-the-Web (`Zone.Identifier`, strefa Internet) — Office, SmartScreen
//! i powłoka traktują go jak plik z Internetu.
//!
//! Implementacja: `platform-windows-sys-impl::DiskDownloads`; atrapa: `FakeDownloads`.

use std::path::{Path, PathBuf};

/// Najdłuższa nazwa pliku po oczyszczeniu (znaki).
pub const MAX_DOWNLOAD_NAME: usize = 120;
/// Nazwa zastępcza, gdy z adresu i nagłówków nie da się ustalić nazwy.
pub const FALLBACK_NAME: &str = "pobrany-plik.bin";

/// Rozszerzenia uruchamialne (wynik oznacza plik; MOTW i tak chroni przy otwarciu).
const EXECUTABLE_EXT: [&str; 28] = [
    "exe",
    "com",
    "scr",
    "pif",
    "bat",
    "cmd",
    "ps1",
    "psm1",
    "vbs",
    "vbe",
    "js",
    "jse",
    "wsf",
    "wsh",
    "hta",
    "msi",
    "msp",
    "msix",
    "appx",
    "appinstaller",
    "lnk",
    "url",
    "reg",
    "dll",
    "cpl",
    "jar",
    "chm",
    "application",
];

/// Nazwy urządzeń Windows (także z rozszerzeniem: `con.txt`).
const DEVICE_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Błąd kwarantanny.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    /// Katalog lub plik niebezpieczny (dowiązanie, junction, poza katalogiem).
    #[error("kwarantanna odrzuciła ścieżkę: {0}")]
    Unsafe(String),
    /// Błąd zapisu.
    #[error("zapis pobrania: {0}")]
    Io(String),
}

/// Zapis jednego pobrania (porzucenie bez [`DownloadSink::commit`] usuwa plik częściowy).
pub trait DownloadSink: Send {
    /// Dopisuje fragment.
    fn write(&mut self, chunk: &[u8]) -> Result<(), DownloadError>;

    /// Kończy: znacznik MOTW z adresem źródła, nazwa końcowa bez nadpisania (kolizja → ` (n)`).
    fn commit(self: Box<Self>, source_url: &str) -> Result<PathBuf, DownloadError>;
}

/// Kwarantanna pobrań.
pub trait DownloadStore: Send + Sync {
    /// Zaczyna zapis pliku `name` (już oczyszczonej) w katalogu `dir` (tworzonym w razie
    /// potrzeby; ścieżka rzeczywista musi być równa podanej).
    fn begin(&self, dir: &Path, name: &str) -> Result<Box<dyn DownloadSink>, DownloadError>;
}

/// Oczyszcza nazwę pliku od serwera albo z adresu: ostatni segment, bez separatorów, ADS (`:`),
/// znaków zabronionych i sterujących, kropek/spacji na końcu, nazw urządzeń; ≤ 120 znaków
/// z zachowaniem rozszerzenia. Pusta → [`FALLBACK_NAME`].
pub fn sanitize_file_name(raw: &str) -> String {
    let last = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = last
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned
        .trim()
        .trim_end_matches(['.', ' '])
        .trim_start_matches('.');
    if cleaned.is_empty() {
        return FALLBACK_NAME.to_owned();
    }
    let (stem, ext) = match cleaned.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() && e.chars().count() <= 16 => (s, Some(e)),
        _ => (cleaned, None),
    };
    let stem_base = stem
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let stem = if DEVICE_NAMES.contains(&stem_base.trim()) {
        format!("_{stem}")
    } else {
        stem.to_owned()
    };
    let room = MAX_DOWNLOAD_NAME.saturating_sub(ext.map_or(0, |e| e.chars().count() + 1));
    let stem: String = stem.chars().take(room.max(1)).collect();
    let stem = stem.trim_end_matches(['.', ' ']);
    let stem = if stem.is_empty() {
        "pobrany-plik"
    } else {
        stem
    };
    match ext {
        Some(e) => format!("{stem}.{e}"),
        None => stem.to_owned(),
    }
}

/// Nazwa z nagłówka `Content-Disposition` (`filename*=UTF-8''…` albo `filename="…"`), jeszcze
/// nieoczyszczona.
pub fn disposition_file_name(header: &str) -> Option<String> {
    let mut plain = None;
    for part in header.split(';').map(str::trim) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "filename*" => {
                let v = value.trim().trim_matches('"');
                let encoded = v.rsplit_once("''").map_or(v, |(_, e)| e);
                if let Some(decoded) = percent_decode(encoded) {
                    return Some(decoded);
                }
            }
            "filename" => plain = Some(value.trim().trim_matches('"').to_owned()),
            _ => {}
        }
    }
    plain.filter(|p| !p.is_empty())
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Czy nazwa ma rozszerzenie uruchamialne.
pub fn is_executable_name(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, e)| EXECUTABLE_EXT.contains(&e.to_ascii_lowercase().as_str()))
}

/// Treść strumienia `Zone.Identifier` (strefa Internet z adresem źródła bez CR/LF).
pub fn zone_identifier(source_url: &str) -> String {
    let url: String = source_url
        .chars()
        .filter(|c| !c.is_control())
        .take(2048)
        .collect();
    format!("[ZoneTransfer]\r\nZoneId=3\r\nHostUrl={url}\r\n")
}

/// Nazwa kandydata przy kolizji: `raport.pdf` → `raport (2).pdf`.
pub fn numbered_name(name: &str, n: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} ({n}).{ext}"),
        _ => format!("{name} ({n})"),
    }
}
