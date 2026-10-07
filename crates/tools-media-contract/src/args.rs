//! Argumenty, wyniki i manifesty `media_info` / `media_convert` / `media_play` oraz czyste
//! reguły: formaty docelowe, zgodność rodzaju wejścia, demuxer wejścia, nazwa nowego pliku.

use lib_media::{MediaInfo, MediaKind};
use risk_classifier_contract::Reversibility;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::{ToolManifest, schema_of};

/// Format docelowy konwersji (lista zamknięta — żadnych dowolnych argumentów ffmpeg).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetFormat {
    /// WAV PCM 16 bit.
    Wav,
    /// MP3 (LAME VBR).
    Mp3,
    /// FLAC.
    Flac,
    /// Ogg Vorbis.
    Ogg,
    /// Ogg Opus.
    Opus,
    /// AAC w M4A.
    M4a,
    /// MP4 (H.264 + AAC).
    Mp4,
    /// WebM (VP9 + Opus).
    Webm,
    /// GIF animowany.
    Gif,
    /// PNG (obraz albo klatka filmu).
    Png,
    /// JPEG (obraz albo klatka filmu).
    Jpg,
}

impl TargetFormat {
    /// Rozszerzenie pliku.
    pub fn ext(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
            Self::M4a => "m4a",
            Self::Mp4 => "mp4",
            Self::Webm => "webm",
            Self::Gif => "gif",
            Self::Png => "png",
            Self::Jpg => "jpg",
        }
    }

    /// Rodzaj wyniku.
    pub fn kind(self) -> MediaKind {
        match self {
            Self::Wav | Self::Mp3 | Self::Flac | Self::Ogg | Self::Opus | Self::M4a => {
                MediaKind::Audio
            }
            Self::Mp4 | Self::Webm => MediaKind::Video,
            Self::Gif | Self::Png | Self::Jpg => MediaKind::Image,
        }
    }

    /// Czy wejście danego rodzaju da się przekonwertować do tego formatu.
    pub fn accepts(self, input: MediaKind) -> bool {
        match self.kind() {
            MediaKind::Audio => matches!(input, MediaKind::Audio | MediaKind::Video),
            MediaKind::Video => input == MediaKind::Video,
            MediaKind::Image => matches!(input, MediaKind::Image | MediaKind::Video),
        }
    }
}

/// Demuxer ffmpeg wymuszany dla formatu wejścia z `lib-media` (`None` — wejście nieobsługiwane;
/// bez demuxerów list odtwarzania, wzorców i protokołów sieciowych).
pub fn input_demuxer(format: &str) -> Option<&'static str> {
    Some(match format {
        "wav" => "wav",
        "mp3" => "mp3",
        "aac" => "aac",
        "flac" => "flac",
        "ogg" => "ogg",
        "mp4" | "mov" | "m4a" | "m4v" | "3gp" | "3g2" => "mov",
        "webm" | "matroska" => "matroska",
        "avi" => "avi",
        "png" => "png_pipe",
        "jpeg" => "jpeg_pipe",
        "gif" => "gif",
        "bmp" => "bmp_pipe",
        "webp" => "webp_pipe",
        _ => return None,
    })
}

fn split_name(path: &str) -> (&str, &str) {
    let cut = path.rfind(['/', '\\']).map_or(0, |i| i + 1);
    let (dir, name) = path.split_at(cut);
    let stem = match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    };
    (dir, stem)
}

/// Nowy plik obok oryginału: `<nazwa> (Alfa).<ext>`, potem `(Alfa 2)`… (`None` — brak wolnej nazwy).
pub fn output_path(original: &str, ext: &str, exists: impl Fn(&str) -> bool) -> Option<String> {
    let (dir, stem) = split_name(original);
    (1..=99)
        .map(|n| {
            if n == 1 {
                format!("{dir}{stem} (Alfa).{ext}")
            } else {
                format!("{dir}{stem} (Alfa {n}).{ext}")
            }
        })
        .find(|p| !exists(p))
}

/// `media_info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InfoArgs {
    /// Plik audio, wideo albo obrazu.
    pub path: String,
}

/// `media_convert`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConvertArgs {
    /// Plik źródłowy.
    pub path: String,
    /// Format docelowy.
    pub format: TargetFormat,
    /// Nowy plik (domyślnie `<nazwa> (Alfa).<ext>` obok źródła); nigdy istniejący.
    #[serde(default)]
    pub output: Option<String>,
    /// Początek fragmentu (s).
    #[serde(default)]
    pub start_s: Option<f64>,
    /// Długość fragmentu (s).
    #[serde(default)]
    pub duration_s: Option<f64>,
    /// Dłuższy bok obrazu/wideo (16–8192; tylko pomniejszanie).
    #[serde(default)]
    pub max_side: Option<u32>,
}

/// Najdłuższy fragment / przesunięcie (s).
pub const MAX_SECONDS: f64 = 24.0 * 3600.0;

fn seconds(v: Option<f64>, name: &str, positive: bool) -> Result<Option<u64>, String> {
    match v {
        None => Ok(None),
        Some(s) if s.is_finite() && s <= MAX_SECONDS && (s > 0.0 || (!positive && s == 0.0)) => {
            Ok(Some((s * 1000.0).round() as u64))
        }
        Some(_) => Err(format!("`{name}` poza zakresem (0–{MAX_SECONDS} s)")),
    }
}

impl ConvertArgs {
    /// Walidacja: ścieżki, czasy, bok, rozszerzenie `output` zgodne z formatem.
    pub fn validate(&self) -> Result<(Option<u64>, Option<u64>), String> {
        if self.path.trim().is_empty() {
            return Err("pusta `path`".into());
        }
        if let Some(out) = &self.output {
            let ext = out.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
            let ok = match ext.as_deref() {
                Some("jpeg") => self.format == TargetFormat::Jpg,
                Some(e) => e == self.format.ext(),
                None => false,
            };
            if out.trim().is_empty() || !ok {
                return Err(format!(
                    "`output` musi kończyć się na .{}",
                    self.format.ext()
                ));
            }
        }
        if self.max_side.is_some_and(|s| !(16..=8192).contains(&s)) {
            return Err("`max_side` poza 16–8192".into());
        }
        Ok((
            seconds(self.start_s, "start_s", false)?,
            seconds(self.duration_s, "duration_s", true)?,
        ))
    }
}

/// `media_play`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlayArgs {
    /// Plik dźwięku (WAV; inne formaty — gdy jest ffmpeg).
    pub path: String,
}

/// Wynik `media_info`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InfoOutput {
    /// Plik.
    pub path: String,
    /// Informacje z nagłówków.
    pub info: MediaInfo,
}

/// Wynik `media_convert`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConvertOutput {
    /// Plik źródłowy (bez zmian).
    pub original: String,
    /// Nowy plik.
    pub output: String,
    /// Format.
    pub format: TargetFormat,
    /// Rozmiar wyniku (B).
    pub bytes: u64,
    /// Krok „Cofnij” w dzienniku.
    pub undo_step: Option<u64>,
}

/// Wynik `media_play`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlayOutput {
    /// Plik.
    pub path: String,
    /// Identyfikator odtwarzania.
    pub playback: u64,
    /// Czas klipu (ms).
    pub duration_ms: u64,
    /// Czeka w kolejce mówienia.
    pub queued: bool,
    /// Plik przekonwertowano do WAV (ffmpeg).
    pub converted: bool,
}

fn manifest(
    name: &str,
    title: &str,
    description: &str,
    (input, output): (serde_json::Value, serde_json::Value),
    capabilities: &[&str],
    group: &str,
    mutating: bool,
) -> ToolManifest {
    ToolManifest {
        name: name.into(),
        id: format!("tools-media.{}", name.trim_start_matches("media_")),
        title: title.into(),
        description: description.into(),
        input_schema: input,
        output_schema: output,
        reversible: Reversibility::Yes,
        capabilities: capabilities.iter().map(|c| (*c).to_owned()).collect(),
        groups: vec!["media".into(), group.into()],
        mutating,
        untrusted_output: None,
    }
}

/// Manifest `media_info`.
pub fn info_manifest() -> ToolManifest {
    manifest(
        "media_info",
        "Informacje o pliku multimedialnym",
        "Odczytuje z nagłówków pliku (bez dekodowania): rodzaj (obraz/dźwięk/wideo), format, kodeki, czas trwania, wymiary, częstotliwość i kanały. Obsługuje PNG, JPEG, GIF, BMP, WebP, WAV, MP3, AAC, FLAC, Ogg, MP4/MOV/M4A, WebM/MKV (częściowo), AVI.",
        (schema_of::<InfoArgs>(), schema_of::<InfoOutput>()),
        &["fs.read"],
        "media.read",
        false,
    )
}

/// Manifest `media_convert`.
pub fn convert_manifest() -> ToolManifest {
    manifest(
        "media_convert",
        "Konwersja multimediów",
        "Konwertuje plik do formatu `format` (wav, mp3, flac, ogg, opus, m4a, mp4, webm, gif, png, jpg) przez ffmpeg, opcjonalnie fragment (`start_s`, `duration_s`) i pomniejszenie (`max_side`). Zapisuje NOWY plik (domyślnie „<nazwa> (Alfa).<ext>” obok źródła) — źródło zostaje bez zmian, krok można cofnąć. Wymaga zainstalowanego ffmpeg (Ustawienia → Modele i silniki).",
        (schema_of::<ConvertArgs>(), schema_of::<ConvertOutput>()),
        &["fs.read", "fs.write"],
        "media.write",
        true,
    )
}

/// Manifest `media_play`.
pub fn play_manifest() -> ToolManifest {
    manifest(
        "media_play",
        "Odtwarzanie dźwięku",
        "Odtwarza plik dźwiękowy na głośniku Alfy w kolejce z mową agentek (najpierw kończy się wypowiedź, mowa użytkownika ścisza i zatrzymuje odtwarzanie). WAV zawsze; inne formaty, gdy jest ffmpeg. Zwraca od razu — odtwarzanie trwa w tle; zatrzymanie przebiegu je przerywa.",
        (schema_of::<PlayArgs>(), schema_of::<PlayOutput>()),
        &["fs.read"],
        "media.play",
        true,
    )
}

/// Manifesty zestawu.
pub fn manifests() -> Vec<ToolManifest> {
    vec![info_manifest(), convert_manifest(), play_manifest()]
}

/// Sprawdza argumenty (te same reguły co implementacja).
pub fn check_args(tool: &str, args: &serde_json::Value) -> Result<(), String> {
    let path_ok = |p: &str| {
        if p.trim().is_empty() {
            Err("pusta `path`".to_owned())
        } else {
            Ok(())
        }
    };
    match tool {
        "media_info" => path_ok(
            &serde_json::from_value::<InfoArgs>(args.clone())
                .map_err(|e| e.to_string())?
                .path,
        ),
        "media_play" => path_ok(
            &serde_json::from_value::<PlayArgs>(args.clone())
                .map_err(|e| e.to_string())?
                .path,
        ),
        "media_convert" => serde_json::from_value::<ConvertArgs>(args.clone())
            .map_err(|e| e.to_string())?
            .validate()
            .map(|_| ()),
        other => Err(format!("nieznane narzędzie {other}")),
    }
}

/// Przykładowe poprawne argumenty.
pub fn sample_args(tool: &str) -> serde_json::Value {
    match tool {
        "media_convert" => serde_json::json!({"path": "film.mp4", "format": "mp3"}),
        _ => serde_json::json!({"path": "nagranie.wav"}),
    }
}
