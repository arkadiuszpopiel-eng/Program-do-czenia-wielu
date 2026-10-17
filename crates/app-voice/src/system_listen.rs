//! Tor słuchania z plików aplikacji: koniec wypowiedzi (Smart Turn albo ocena z tekstu)
//! i rozpoznawanie mowy (Qwen3-ASR ze strażnikiem i Whisperem w zapasie albo sam Whisper) —
//! z rozgrzewką i zwalnianiem silnika po bezczynności.

use std::sync::Arc;
use std::time::Duration;

use voice_pipeline_contract::PipelineError;
use voice_stt_contract::Stt;
use voice_stt_impl::ProcessLauncher;
use voice_stt_impl::fallback::GuardedStt;
use voice_stt_impl::qwen::QwenStt;
use voice_turn_contract::TurnDetector;
use voice_turn_impl::{HeuristicTurnModel, PatienceTurnDetector, SmartTurnModel};

use crate::system::SystemVoice;

/// Co minutę sprawdza, czy silnik Qwen3-ASR stoi bez użycia dłużej niż jego limit — wtedy go
/// zamyka i zwalnia pamięć karty. Zadanie kończy się samo, gdy silnik przestaje istnieć.
fn idle_unload(qwen: &Arc<QwenStt>) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    // Rozgrzewka: model wczytuje się w tle od startu rozmowy, nie przy pierwszym zdaniu.
    let warm = qwen.clone();
    runtime.spawn(async move {
        if let Err(e) = warm.warm().await {
            tracing::warn!(error = %e, "Qwen3-ASR: rozgrzewka nieudana (spróbuje przy pierwszej wypowiedzi)");
        }
    });
    let weak = Arc::downgrade(qwen);
    runtime.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let Some(engine) = weak.upgrade() else {
                break;
            };
            engine.unload_if_idle().await;
        }
    });
}

impl SystemVoice {
    /// Koniec wypowiedzi: model Smart Turn na dźwięku (z czasami strojonymi do rozmowy), gdy są
    /// jego pliki; inaczej model tekstowy z domyślną cierpliwością.
    pub(crate) fn turn(&self) -> Box<dyn TurnDetector> {
        let sidecar = self
            .talk
            .smart_turn()
            .then(|| app_modules::turn::smart_turn(&self.paths))
            .flatten();
        let Some(sidecar) = sidecar else {
            return Box::new(PatienceTurnDetector::new(HeuristicTurnModel));
        };
        let cfg = self.talk.turn_cfg();
        match PatienceTurnDetector::with_cfg(
            SmartTurnModel::sidecar(sidecar).with_stats(self.turn_stats.clone()),
            cfg,
        ) {
            Ok(detector) => Box::new(detector),
            Err(e) => {
                tracing::warn!(error = %e, "Smart Turn niedostępny — koniec wypowiedzi z tekstu");
                Box::new(PatienceTurnDetector::new(HeuristicTurnModel))
            }
        }
    }

    /// Rozpoznawanie mowy dla rozmowy i dyktowania: Qwen3-ASR ze strażnikiem i Whisperem jako
    /// zapasem, gdy model Qwen3-ASR jest pobrany; inaczej sam Whisper.
    pub(crate) fn stt(&self) -> Result<Arc<dyn Stt>, PipelineError> {
        let stt = self.compose_stt()?;
        // Język z Ustawień trafia do silnika od razu po złożeniu (zanim padnie pierwsze zdanie).
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let cfg = voice_stt_contract::SttCfg {
                language: self.talk.language(),
                ..voice_stt_contract::SttCfg::default()
            };
            let engine = stt.clone();
            runtime.spawn(async move {
                if let Err(e) = engine.configure(cfg).await {
                    tracing::warn!(error = %e, "rozpoznawanie mowy: nie udało się ustawić języka");
                }
            });
        }
        Ok(stt)
    }

    /// Silnik rozpoznawania dobrany do plików, ustawień i wolnej pamięci karty.
    fn compose_stt(&self) -> Result<Arc<dyn Stt>, PipelineError> {
        let whisper: Arc<dyn Stt> = Arc::new(self.whisper()?);
        if self.talk.whisper_only() {
            return Ok(whisper);
        }
        // Silnik i model są na miejscu? (bez pomiaru: flaga `gpu` mówi, czy to kompilacja na kartę)
        let Some(mut config) = app_modules::stt::qwen(&self.paths, None) else {
            return Ok(whisper);
        };
        // Pomiar wolnej pamięci karty: gdy zajął ją model rozmowy, wybieramy silnik, który da radę.
        let free = app_modules::gpu_mem::free_vram_mb();
        let plan = app_modules::stt::stt_plan(
            config.gpu,
            app_modules::stt::server_cuda(&self.paths).is_some(),
            free,
        );
        tracing::info!(?plan, free_vram_mb = ?free, "rozpoznawanie mowy: wybór silnika");
        *self
            .stt_plan
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(plan);
        if plan == app_modules::stt::SttPlan::WhisperGpu {
            return Ok(whisper);
        }
        config.gpu = plan == app_modules::stt::SttPlan::QwenGpu;
        match QwenStt::new(config, Arc::new(ProcessLauncher)) {
            Ok(qwen) => {
                let qwen = Arc::new(qwen);
                idle_unload(&qwen);
                let guarded = Arc::new(GuardedStt::new(qwen, whisper));
                *self
                    .guard
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(guarded.clone());
                Ok(guarded)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Qwen3-ASR niedostępny — rozpoznaje Whisper");
                Ok(whisper)
            }
        }
    }
}
