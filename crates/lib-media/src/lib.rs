//! Czyste parsery nagłówków multimediów (bez ffmpeg, bez dekodowania pikseli i próbek) dla
//! `tools-media` (`media_info`) i `tools-vision` (wymiary obrazu przed OCR — ochrona przed
//! „bombą dekompresyjną”).
//!
//! Wejście jest **niezaufane**: każdy odczyt idzie przez [`Reader`] z budżetem bajtów i liczbą
//! kroków ([`Limits`]), cała arytmetyka na rozmiarach jest sprawdzana, a parser nigdy nie panikuje
//! (testy właściwości na losowych i zmutowanych plikach). Odczyt fragmentów pliku ([`RangeRead`])
//! nie wczytuje całości — nagłówek MP4 na końcu wielogigabajtowego filmu kosztuje kilka odczytów.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod audio;
mod image;
mod mp4;
#[cfg(any(test, feature = "samples"))]
pub mod samples;
mod sniff;
mod source;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use sniff::{Format, sniff};
pub use source::{ByteSource, FileSource, PortFiles, RangeRead, SliceSource, StdFiles};

/// Rodzaj multimediów.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    /// Obraz (także animowany).
    Image,
    /// Dźwięk.
    Audio,
    /// Wideo (może mieć ścieżkę dźwięku).
    Video,
}

/// Informacje z nagłówków pliku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MediaInfo {
    /// Rodzaj.
    pub kind: MediaKind,
    /// Format kontenera (`png`, `jpeg`, `mp4`, `mov`, `m4a`, `wav`, `mp3`, `ogg`…).
    pub format: String,
    /// Typ MIME.
    pub mime: String,
    /// Rozmiar pliku (B).
    pub size_bytes: u64,
    /// Szerokość (px).
    pub width: Option<u32>,
    /// Wysokość (px).
    pub height: Option<u32>,
    /// Czas trwania (ms).
    pub duration_ms: Option<u64>,
    /// Częstotliwość próbkowania (Hz).
    pub sample_rate: Option<u32>,
    /// Liczba kanałów (dźwięk) albo składowych koloru (obraz).
    pub channels: Option<u16>,
    /// Bity na próbkę / składową.
    pub bits_per_sample: Option<u16>,
    /// Przepływność (b/s), jeśli wynika z nagłówka.
    pub bit_rate: Option<u64>,
    /// Liczba klatek (animacja, wideo), jeśli wynika z nagłówka.
    pub frames: Option<u32>,
    /// Kodeki (np. `h264`, `aac`, `pcm_s16le`).
    pub codecs: Vec<String>,
    /// Nagłówek rozpoznany tylko częściowo (część pól pominięta albo limit odczytu).
    pub partial: bool,
}

impl MediaInfo {
    /// Pusty opis formatu.
    pub fn new(kind: MediaKind, format: &str, mime: &str, size_bytes: u64) -> Self {
        Self {
            kind,
            format: format.to_owned(),
            mime: mime.to_owned(),
            size_bytes,
            width: None,
            height: None,
            duration_ms: None,
            sample_rate: None,
            channels: None,
            bits_per_sample: None,
            bit_rate: None,
            frames: None,
            codecs: Vec::new(),
            partial: false,
        }
    }

    /// Liczba pikseli (szerokość × wysokość), jeśli znana.
    pub fn pixels(&self) -> Option<u64> {
        Some(u64::from(self.width?) * u64::from(self.height?))
    }
}

/// Błąd rozpoznania.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MediaError {
    /// Format nierozpoznany albo nieobsługiwany.
    #[error("nierozpoznany albo nieobsługiwany format pliku")]
    Unknown,
    /// Plik kończy się przed potrzebnym nagłówkiem.
    #[error("plik ucięty ({0})")]
    Truncated(&'static str),
    /// Nagłówek niespójny.
    #[error("uszkodzony nagłówek: {0}")]
    Malformed(String),
    /// Przekroczony budżet odczytu, liczba kroków albo głębokość zagnieżdżenia.
    #[error("przekroczony limit analizy: {0}")]
    Limit(String),
    /// Błąd odczytu.
    #[error("odczyt pliku: {0}")]
    Io(String),
}

/// Limity analizy jednego pliku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Ile bajtów wolno przeczytać ze źródła (z odczytami blokowymi).
    pub max_read_bytes: u64,
    /// Ile kroków (pudełek, bloków, segmentów) wolno odwiedzić.
    pub max_steps: u32,
    /// Maksymalna głębokość zagnieżdżenia pudełek MP4.
    pub max_depth: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_read_bytes: 8 * 1024 * 1024,
            max_steps: 100_000,
            max_depth: 10,
        }
    }
}

/// Rozpoznaje plik ze źródła (sygnatura → parser formatu) w limitach.
pub fn probe(src: &mut dyn ByteSource, limits: &Limits) -> Result<MediaInfo, MediaError> {
    let mut r = Reader::new(src, *limits);
    let head = r.get(0, 64)?;
    let format = sniff(&head).ok_or(MediaError::Unknown)?;
    match format {
        Format::Png => image::png(&mut r),
        Format::Jpeg => image::jpeg(&mut r),
        Format::Gif => image::gif(&mut r),
        Format::Bmp => image::bmp(&mut r),
        Format::Webp => image::webp(&mut r),
        Format::Wav => audio::wav(&mut r),
        Format::Mp3 => audio::mp3(&mut r),
        Format::Aac => audio::aac(&mut r),
        Format::Flac => audio::flac(&mut r),
        Format::Ogg => audio::ogg(&mut r),
        Format::Mp4 => mp4::mp4(&mut r),
        Format::Matroska => mp4::matroska(&mut r),
        Format::Avi => mp4::avi(&mut r),
    }
}

/// Rozpoznaje plik w pamięci (domyślne limity).
pub fn probe_bytes(bytes: &[u8]) -> Result<MediaInfo, MediaError> {
    probe(&mut SliceSource::new(bytes), &Limits::default())
}

/// Odczyt z budżetem i pamięcią podręczną ostatniego bloku (nagłówki leżą blisko siebie).
pub(crate) struct Reader<'a> {
    src: &'a mut dyn ByteSource,
    size: u64,
    budget: u64,
    steps: u32,
    block: (u64, Vec<u8>),
    pub(crate) limits: Limits,
}

const BLOCK: u64 = 64 * 1024;

impl<'a> Reader<'a> {
    pub(crate) fn new(src: &'a mut dyn ByteSource, limits: Limits) -> Self {
        let size = src.size();
        Self {
            src,
            size,
            budget: limits.max_read_bytes,
            steps: limits.max_steps,
            block: (0, Vec::new()),
            limits,
        }
    }

    /// Rozmiar źródła.
    pub(crate) fn size(&self) -> u64 {
        self.size
    }

    /// Do `len` bajtów od `offset` (mniej na końcu pliku, pusto poza nim).
    pub(crate) fn get(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, MediaError> {
        if offset >= self.size || len == 0 {
            return Ok(Vec::new());
        }
        let end = offset.saturating_add(len as u64).min(self.size);
        let (start, data) = &self.block;
        let cached_end = start.saturating_add(data.len() as u64);
        if offset >= *start && end <= cached_end {
            let (a, b) = ((offset - start) as usize, (end - start) as usize);
            return Ok(data.get(a..b).map(<[u8]>::to_vec).unwrap_or_default());
        }
        let want = (end - offset).max(BLOCK).min(self.size - offset);
        if want > self.budget {
            return Err(MediaError::Limit(format!(
                "odczyt ponad {} B",
                self.limits.max_read_bytes
            )));
        }
        self.budget -= want;
        let bytes = self.src.read_at(offset, want as usize)?;
        let n = ((end - offset) as usize).min(bytes.len());
        let out = bytes.get(..n).map(<[u8]>::to_vec).unwrap_or_default();
        self.block = (offset, bytes);
        Ok(out)
    }

    /// Dokładnie `len` bajtów od `offset` albo [`MediaError::Truncated`].
    pub(crate) fn exact(
        &mut self,
        offset: u64,
        len: usize,
        what: &'static str,
    ) -> Result<Vec<u8>, MediaError> {
        let bytes = self.get(offset, len)?;
        if bytes.len() < len {
            return Err(MediaError::Truncated(what));
        }
        Ok(bytes)
    }

    /// Jeden krok pętli parsera (pudełko, blok, segment).
    pub(crate) fn step(&mut self) -> Result<(), MediaError> {
        if self.steps == 0 {
            return Err(MediaError::Limit(format!(
                "ponad {} elementów struktury",
                self.limits.max_steps
            )));
        }
        self.steps -= 1;
        Ok(())
    }
}

pub(crate) fn malformed(what: &str) -> MediaError {
    MediaError::Malformed(what.to_owned())
}

fn field<const N: usize>(b: &[u8], i: usize) -> Result<[u8; N], MediaError> {
    b.get(i..i.saturating_add(N))
        .and_then(|s| s.try_into().ok())
        .ok_or(MediaError::Truncated("pole nagłówka"))
}

pub(crate) fn be16(b: &[u8], i: usize) -> Result<u16, MediaError> {
    field::<2>(b, i).map(u16::from_be_bytes)
}

pub(crate) fn be32(b: &[u8], i: usize) -> Result<u32, MediaError> {
    field::<4>(b, i).map(u32::from_be_bytes)
}

pub(crate) fn be64(b: &[u8], i: usize) -> Result<u64, MediaError> {
    field::<8>(b, i).map(u64::from_be_bytes)
}

pub(crate) fn le16(b: &[u8], i: usize) -> Result<u16, MediaError> {
    field::<2>(b, i).map(u16::from_le_bytes)
}

pub(crate) fn le32(b: &[u8], i: usize) -> Result<u32, MediaError> {
    field::<4>(b, i).map(u32::from_le_bytes)
}

pub(crate) fn le64(b: &[u8], i: usize) -> Result<u64, MediaError> {
    field::<8>(b, i).map(u64::from_le_bytes)
}

pub(crate) fn byte(b: &[u8], i: usize) -> Result<u8, MediaError> {
    b.get(i)
        .copied()
        .ok_or(MediaError::Truncated("pole nagłówka"))
}

/// `units` jednostek przy `per_second` jednostkach na sekundę → milisekundy (bez przepełnienia).
pub(crate) fn to_ms(units: u64, per_second: u64) -> Option<u64> {
    if per_second == 0 {
        return None;
    }
    let ms = u128::from(units) * 1000 / u128::from(per_second);
    u64::try_from(ms).ok()
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_fuzz;
