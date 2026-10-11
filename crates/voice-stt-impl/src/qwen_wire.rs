//! Qwen3-ASR — części bez we/wy: konfiguracja, argumenty `llama-server`, nagranie jako WAV,
//! odczyt odpowiedzi modelu i podpowiedź nazw własnych.

use std::path::PathBuf;
use std::time::Duration;

/// Nazwa modelu w zdarzeniach.
pub const MODEL_NAME: &str = "Qwen3-ASR-1.7B";
/// Najdłuższy klip wysyłany do modelu (z końca nagrania).
const MAX_CLIP_SAMPLES: usize = 30 * 16_000;
/// Najwięcej nazw własnych w podpowiedzi.
const MAX_TERMS: usize = 24;

/// Konfiguracja silnika.
#[derive(Debug, Clone)]
pub struct QwenConfig {
    /// `llama-server` (wydanie obsługujące dźwięk — b11559 lub nowsze).
    pub server: PathBuf,
    /// Plik modelu (`Qwen3-ASR-1.7B-Q8_0.gguf`).
    pub model: PathBuf,
    /// Projektor dźwięku (`mmproj-Qwen3-ASR-1.7B-Q8_0.gguf`).
    pub mmproj: PathBuf,
    /// Czy liczyć na karcie graficznej (inaczej procesor).
    pub gpu: bool,
    /// Wątki procesora.
    pub threads: usize,
    /// Najdłuższe czekanie na gotowość serwera.
    pub startup: Duration,
    /// Limit czasu jednego rozpoznania.
    pub request_timeout: Duration,
    /// Po tylu sekundach bez użycia proces jest zamykany (zwalnia pamięć karty).
    pub idle_unload: Duration,
}

impl QwenConfig {
    /// Konfiguracja z wartościami z Delty: start do 120 s, zapytanie do 90 s, zwolnienie po 10 min.
    pub fn new(server: PathBuf, model: PathBuf, mmproj: PathBuf, gpu: bool) -> Self {
        let cores = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
        Self {
            server,
            model,
            mmproj,
            gpu,
            threads: (cores / 2).max(4),
            startup: Duration::from_secs(120),
            request_timeout: Duration::from_secs(90),
            idle_unload: Duration::from_secs(600),
        }
    }
}

/// Argumenty `llama-server` dla Qwen3-ASR.
pub fn build_args(cfg: &QwenConfig, port: u16) -> Vec<String> {
    let path = |p: &PathBuf| p.to_string_lossy().into_owned();
    let mut args = vec![
        "--model".to_owned(),
        path(&cfg.model),
        "--mmproj".to_owned(),
        path(&cfg.mmproj),
        "--ctx-size".to_owned(),
        "4096".to_owned(),
        "--n-gpu-layers".to_owned(),
        if cfg.gpu { "-1" } else { "0" }.to_owned(),
        "--threads".to_owned(),
        cfg.threads.to_string(),
        "--host".to_owned(),
        "127.0.0.1".to_owned(),
        "--port".to_owned(),
        port.to_string(),
        "--parallel".to_owned(),
        "1".to_owned(),
        "--jinja".to_owned(),
        "--no-warmup".to_owned(),
    ];
    if !cfg.gpu {
        args.push("--no-mmproj-offload".to_owned());
    }
    args
}

/// Próbki 16 kHz mono → plik WAV PCM16 (ostatnie 30 s nagrania).
pub fn wav16(samples: &[f32]) -> Vec<u8> {
    let clip = &samples[samples.len().saturating_sub(MAX_CLIP_SAMPLES)..];
    let data_len = u32::try_from(clip.len() * 2).unwrap_or(u32::MAX);
    let mut wav = Vec::with_capacity(44 + clip.len() * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&16_000u32.to_le_bytes());
    wav.extend_from_slice(&32_000u32.to_le_bytes()); // bajtów na sekundę
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in clip {
        #[allow(clippy::cast_possible_truncation)]
        let v = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        wav.extend_from_slice(&v.to_le_bytes());
    }
    wav
}

/// Odpowiedź modelu `language Polish<asr_text>tekst` → (kod języka, tekst). Bez znacznika cała
/// odpowiedź jest tekstem, a język pozostaje nieznany.
pub fn parse_reply(content: &str) -> (String, String) {
    let content = content.trim();
    let Some((head, text)) = content.split_once("<asr_text>") else {
        return (String::new(), content.to_owned());
    };
    let name = head.trim().strip_prefix("language").unwrap_or(head).trim();
    (crate::client::iso_lang(name), text.trim().to_owned())
}

/// Tekst podpowiedzi z nazwami własnymi (najwyżej 24) — poprawia zapis imion i nazw.
pub fn terms_prompt(terms: &[String]) -> Option<String> {
    let list: Vec<&str> = terms
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .take(MAX_TERMS)
        .collect();
    (!list.is_empty()).then(|| format!("Nazwy własne, które mogą paść: {}", list.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(gpu: bool) -> QwenConfig {
        QwenConfig::new(
            "llama-server".into(),
            "model.gguf".into(),
            "mmproj.gguf".into(),
            gpu,
        )
    }

    #[test]
    fn arguments_for_gpu_and_cpu() {
        let gpu = build_args(&config(true), 8082);
        let at = |args: &[String], flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| args[i + 1].clone())
        };
        assert_eq!(at(&gpu, "--n-gpu-layers").as_deref(), Some("-1"));
        assert_eq!(at(&gpu, "--mmproj").as_deref(), Some("mmproj.gguf"));
        assert_eq!(at(&gpu, "--ctx-size").as_deref(), Some("4096"));
        assert_eq!(at(&gpu, "--host").as_deref(), Some("127.0.0.1"));
        assert_eq!(at(&gpu, "--port").as_deref(), Some("8082"));
        assert!(gpu.contains(&"--jinja".to_owned()) && gpu.contains(&"--no-warmup".to_owned()));
        assert!(!gpu.contains(&"--no-mmproj-offload".to_owned()));
        let cpu = build_args(&config(false), 1);
        assert_eq!(at(&cpu, "--n-gpu-layers").as_deref(), Some("0"));
        assert!(cpu.contains(&"--no-mmproj-offload".to_owned()));
        assert!(config(true).threads >= 4);
    }

    #[test]
    fn reply_is_split_into_language_and_text() {
        assert_eq!(
            parse_reply("language Polish<asr_text>Dzień dobry, tu Alfa."),
            ("pl".to_owned(), "Dzień dobry, tu Alfa.".to_owned())
        );
        assert_eq!(parse_reply(" language English<asr_text> Hello. ").0, "en");
        assert_eq!(
            parse_reply("sam tekst"),
            (String::new(), "sam tekst".to_owned())
        );
        assert_eq!(parse_reply("language Polish<asr_text>").1, "");
    }

    #[test]
    fn wav_header_and_thirty_second_limit() {
        let wav = wav16(&[0.0, 1.0, -1.0, 2.0]);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            16_000
        );
        assert_eq!(wav.len(), 44 + 8);
        assert_eq!(&wav[46..52], &[0xFF, 0x7F, 0x01, 0x80, 0xFF, 0x7F]);
        let long = wav16(&vec![0.1; 40 * 16_000]);
        assert_eq!(long.len(), 44 + 30 * 16_000 * 2, "ostatnie 30 s");
    }

    #[test]
    fn proper_names_prompt_is_capped() {
        assert_eq!(terms_prompt(&[]), None);
        assert_eq!(terms_prompt(&[" ".into()]), None);
        let many: Vec<String> = (0..40).map(|i| format!("Nazwa{i}")).collect();
        let prompt = terms_prompt(&many).unwrap_or_default();
        assert!(prompt.starts_with("Nazwy własne, które mogą paść: Nazwa0, Nazwa1"));
        assert!(prompt.contains("Nazwa23") && !prompt.contains("Nazwa24"));
    }
}
