//! „Sprawdź silniki”: próba każdego silnika rozmowy bez mikrofonu i bez odtwarzania — rozpoznawanie
//! (wczytanie modelu), koniec wypowiedzi (ocena sekundy szumu) i głos (start procesu z próbną
//! syntezą, wynik odrzucany). Wynik z czasem trafia do widoku silników.

use std::sync::Arc;
use std::time::Instant;

use app_api::dto::{EngineCheckView, SelfTestView};
use voice_stt_impl::ProcessLauncher;
use voice_stt_impl::qwen::QwenStt;
use voice_turn_impl::{AudioScorer, SidecarScorer};

use crate::system::SystemVoice;

fn elapsed_ms(started: Instant) -> u32 {
    u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX)
}

fn passed(started: Instant, detail: Option<&str>) -> EngineCheckView {
    EngineCheckView {
        ok: true,
        ms: elapsed_ms(started),
        detail: detail.map(str::to_owned),
    }
}

fn failed(started: Instant, detail: impl std::fmt::Display) -> EngineCheckView {
    EngineCheckView {
        ok: false,
        ms: elapsed_ms(started),
        detail: Some(detail.to_string()),
    }
}

/// Sekunda cichego szumu 16 kHz (deterministyczna) — dość, żeby model końca wypowiedzi policzył wynik.
fn probe_noise() -> Vec<f32> {
    let mut state = 0x2545_F491_u32;
    (0..16_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            #[allow(clippy::cast_precision_loss)]
            let unit = (state >> 8) as f32 / 16_777_216.0;
            (unit - 0.5) * 0.02
        })
        .collect()
}

impl SystemVoice {
    async fn check_tts(&self) -> EngineCheckView {
        let started = Instant::now();
        let Some(tts) = self.tts_ready() else {
            return failed(
                started,
                "brak silnika mowy — pobierz głos w „Modele lokalne i silniki”",
            );
        };
        match tts.warm(&personas_contract::PersonaId::alfa()).await {
            Ok(()) => passed(started, None),
            Err(e) => failed(started, e),
        }
    }

    async fn check_turn(&self) -> EngineCheckView {
        let started = Instant::now();
        if !self.talk.smart_turn() {
            return passed(
                started,
                Some("Smart Turn wyłączony w ustawieniach — ocena z tekstu"),
            );
        }
        let Some(sidecar) = app_modules::turn::smart_turn(&self.paths) else {
            return failed(started, "brak plików Smart Turn — działa ocena z tekstu");
        };
        let scored =
            tokio::task::spawn_blocking(move || SidecarScorer::new(sidecar).score(&probe_noise()))
                .await;
        match scored {
            Ok(Ok(_)) => passed(started, None),
            Ok(Err(e)) => failed(started, e),
            Err(e) => failed(started, e),
        }
    }

    async fn check_stt(&self) -> EngineCheckView {
        let started = Instant::now();
        if !self.whisper_server().is_file() || self.whisper_model().is_none() {
            return failed(started, "brak silnika albo modelu Whisper (wymagany zapas)");
        }
        let qwen = (!self.talk.whisper_only())
            .then(|| app_modules::stt::qwen(&self.paths, None))
            .flatten();
        let Some(mut config) = qwen else {
            return passed(
                started,
                Some("Whisper: pliki na miejscu (silnik startuje przy pierwszej wypowiedzi)"),
            );
        };
        // Próba nie zabiera karty trwającej rozmowie: na karcie tylko przy wolnej pamięci.
        let free = tokio::task::spawn_blocking(app_modules::gpu_mem::nvidia_free_mb)
            .await
            .ok()
            .flatten();
        config.gpu = app_modules::stt::qwen_on_gpu(config.gpu, free);
        let on_card = config.gpu;
        let engine = match QwenStt::new(config, Arc::new(ProcessLauncher)) {
            Ok(engine) => engine,
            Err(e) => return failed(started, e),
        };
        // Silnik próbny kończy pracę razem z tą funkcją (zwalnia pamięć).
        match engine.warm().await {
            Ok(()) => passed(
                started,
                Some(if on_card {
                    "Qwen3-ASR wczytany na karcie graficznej"
                } else {
                    "Qwen3-ASR wczytany na procesorze (karta zajęta albo brak karty)"
                }),
            ),
            Err(e) => failed(started, e),
        }
    }

    /// Uruchamia próby po kolei (żeby nie mierzyć się nawzajem) i zapamiętuje wynik.
    pub(crate) async fn run_self_test(&self) {
        let turn = self.check_turn().await;
        let tts = self.check_tts().await;
        let stt = self.check_stt().await;
        *self
            .self_test
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some(SelfTestView { stt, turn, tts });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_noise_is_quiet_deterministic_and_one_second_long() {
        let a = probe_noise();
        assert_eq!(a.len(), 16_000);
        assert_eq!(a, probe_noise());
        assert!(a.iter().all(|s| s.abs() <= 0.011));
        assert!(a.iter().any(|s| s.abs() > 0.001));
    }

    #[test]
    fn results_carry_time_and_reason() {
        let started = Instant::now();
        let ok = passed(started, Some("szczegół"));
        assert!(ok.ok && ok.detail.as_deref() == Some("szczegół"));
        let bad = failed(started, "powód");
        assert!(!bad.ok && bad.detail.as_deref() == Some("powód"));
    }
}
