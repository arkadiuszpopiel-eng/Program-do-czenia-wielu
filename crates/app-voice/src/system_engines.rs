//! Podgląd silników rozmowy dla Ustawień → Głos: który silnik rozpoznaje mowę, stan strażnika
//! i bezpiecznika, co ocenia koniec wypowiedzi, które głosy są gotowe. Odczyt z plików aplikacji
//! (bez uruchamiania silników) i z ostatnio złożonego rozpoznawania.

use app_api::dto::{
    BreakerStateView, SttEngineKind, SttEngineView, SttGuardView, TtsEngineView, TurnEngineKind,
    TurnEngineView, TurnStatsView, VoiceEnginesView,
};
use voice_stt_impl::breaker::BreakerState;
use voice_stt_impl::fallback::GuardStatus;

use crate::system::SystemVoice;

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Stan strażnika w postaci dla okna.
pub(crate) fn guard_view(status: &GuardStatus) -> SttGuardView {
    let (samples, fallbacks, flagged) = status.counts;
    SttGuardView {
        breaker: match status.breaker {
            BreakerState::Closed => BreakerStateView::Closed,
            BreakerState::Open => BreakerStateView::Open,
            BreakerState::HalfOpen => BreakerStateView::HalfOpen,
        },
        failures: status.failures,
        failures_to_open: status.failures_to_open,
        opened_count: status.opened_count,
        retry_in_s: status.retry_in_s,
        median_ms: status.processing_ms.map(|(median, _)| median),
        p95_ms: status.processing_ms.map(|(_, p95)| p95),
        samples: count(samples),
        fallbacks: count(fallbacks),
        flagged: count(flagged),
    }
}

/// Liczniki modelu końca wypowiedzi w postaci dla okna (`None` — nic jeszcze nie oceniał).
pub(crate) fn turn_stats_view(stats: voice_turn_impl::TurnStatsSnapshot) -> Option<TurnStatsView> {
    (stats.scored > 0 || stats.failed > 0).then(|| TurnStatsView {
        scored: stats.scored,
        failed: stats.failed,
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        last_pct: stats
            .last_probability
            .map(|p| (p.clamp(0.0, 1.0) * 100.0).round() as u8),
        last_ms: stats.last_ms,
    })
}

impl SystemVoice {
    /// Silniki rozmowy na tej maszynie.
    pub(crate) fn engines_view(&self) -> VoiceEnginesView {
        let whisper = self.whisper_server().is_file() && self.whisper_model().is_some();
        // Bez pomiaru pamięci karty (tani odczyt z plików); faktyczny wybór z ostatniego złożenia.
        let qwen = (!self.talk.whisper_only())
            .then(|| app_modules::stt::qwen(&self.paths, None))
            .flatten();
        let plan = *self
            .stt_plan
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Karta zajęta przez inny model: ostatnio rozpoznawał sam Whisper na karcie.
        let qwen = qwen.filter(|_| plan != Some(app_modules::stt::SttPlan::WhisperGpu));
        let guard = self
            .guard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|g| guard_view(&g.status()));
        let stt = match (&qwen, whisper) {
            (Some(cfg), true) => SttEngineView {
                engine: SttEngineKind::QwenAsr,
                gpu: plan.map_or(cfg.gpu, |p| p == app_modules::stt::SttPlan::QwenGpu),
                fallback: Some(SttEngineKind::Whisper),
                missing: Vec::new(),
                guard,
            },
            (_, true) => SttEngineView {
                engine: SttEngineKind::Whisper,
                gpu: self
                    .paths
                    .sidecar("whisper-cuda", "whisper-server")
                    .is_file(),
                fallback: None,
                // Wybór „tylko Whisper” to decyzja użytkownika — nic nie brakuje.
                missing: if self.talk.whisper_only()
                    || plan == Some(app_modules::stt::SttPlan::WhisperGpu)
                {
                    Vec::new()
                } else {
                    app_modules::stt::qwen_missing(&self.paths)
                },
                guard: None,
            },
            // Bez Whispera rozmowa nie rusza (jest wymaganym zapasem) — także z modelem Qwen3-ASR.
            (_, false) => SttEngineView {
                missing: vec!["sidecar-whisper-cpu".into(), "whisper-small-q5_1".into()],
                ..SttEngineView::default()
            },
        };
        let turn_missing = if self.talk.smart_turn() {
            app_modules::turn::smart_turn_missing(&self.paths)
        } else {
            Vec::new()
        };
        let turn = TurnEngineView {
            engine: if turn_missing.is_empty() && self.talk.smart_turn() {
                TurnEngineKind::SmartTurn
            } else {
                TurnEngineKind::Text
            },
            missing: turn_missing,
            stats: turn_stats_view(self.turn_stats.snapshot()),
        };
        let tts = app_modules::tts::installed(&self.paths)
            .into_iter()
            .map(|(engine, ready)| TtsEngineView {
                engine: engine.to_owned(),
                ready,
            })
            .collect();
        VoiceEnginesView { stt, turn, tts }
    }

    /// Zamyka bezpiecznik ostatnio złożonego rozpoznawania.
    pub(crate) fn reset_guard(&self) {
        if let Some(guard) = self
            .guard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            guard.reset_breaker();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_status_maps_to_the_view() {
        let view = guard_view(&GuardStatus {
            breaker: BreakerState::Open,
            failures: 3,
            failures_to_open: 3,
            opened_count: 2,
            retry_in_s: Some(17),
            processing_ms: Some((280, 910)),
            counts: (40, 5, 2),
        });
        assert_eq!(view.breaker, BreakerStateView::Open);
        assert_eq!((view.failures, view.failures_to_open), (3, 3));
        assert_eq!((view.opened_count, view.retry_in_s), (2, Some(17)));
        assert_eq!((view.median_ms, view.p95_ms), (Some(280), Some(910)));
        assert_eq!((view.samples, view.fallbacks, view.flagged), (40, 5, 2));
        let idle = guard_view(&GuardStatus {
            breaker: BreakerState::Closed,
            failures: 0,
            failures_to_open: 3,
            opened_count: 0,
            retry_in_s: None,
            processing_ms: None,
            counts: (0, 0, 0),
        });
        assert_eq!(idle.breaker, BreakerStateView::Closed);
        assert_eq!((idle.median_ms, idle.p95_ms), (None, None));
    }

    #[test]
    fn turn_stats_appear_only_after_the_first_score() {
        use voice_turn_impl::TurnStatsSnapshot;
        assert_eq!(turn_stats_view(TurnStatsSnapshot::default()), None);
        let view = turn_stats_view(TurnStatsSnapshot {
            scored: 7,
            failed: 1,
            last_probability: Some(0.934),
            last_ms: Some(26),
        })
        .unwrap();
        assert_eq!((view.scored, view.failed), (7, 1));
        assert_eq!((view.last_pct, view.last_ms), (Some(93), Some(26)));
        let failed_only = turn_stats_view(TurnStatsSnapshot {
            failed: 2,
            ..TurnStatsSnapshot::default()
        })
        .unwrap();
        assert_eq!((failed_only.scored, failed_only.last_pct), (0, None));
    }
}
