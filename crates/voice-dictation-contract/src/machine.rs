//! Deterministyczny automat sesji dyktowania (wspólny dla `-impl` i `-fake`): cel = okno, które
//! było na pierwszym planie w chwili startu; zmiana okna → pauza (tekst czeka), powrót → wznowienie;
//! frazy → normalizacja → kolejka do wpisania; historia wpisanych fraz dla „cofnij to” (tylko
//! ostatnia fraza, w oknie czasu, bez zmiany okna i bez wejścia użytkownika od wpisania).

use std::collections::VecDeque;

use crate::normalize::{TextContext, normalize};
use crate::{
    DictationCfg, DictationEvent, DictationMode, DictationPhase, DictationStatus, DictationTarget,
    PauseReason, StopReason, control_command,
};

/// Polecenie automatu do wykonania przez usługę.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationAction {
    /// Wpisz tekst (fraza `id`) do okna celu.
    Type {
        /// Fraza.
        id: u64,
        /// Tekst.
        text: String,
    },
    /// Usuń `chars` znaków przed kursorem (Backspace) — „cofnij to”.
    Erase {
        /// Fraza.
        id: u64,
        /// Liczba znaków.
        chars: usize,
    },
}

#[derive(Debug, Clone)]
struct Typed {
    id: u64,
    chars: usize,
    at_ms: u64,
    ctx_before: TextContext,
}

/// Automat sesji.
#[derive(Debug, Clone)]
pub struct DictationMachine {
    cfg: DictationCfg,
    phase: DictationPhase,
    target: Option<DictationTarget>,
    mode: DictationMode,
    ctx: TextContext,
    pending: VecDeque<(u64, String)>,
    /// Kontekst tekstu sprzed każdej niewpisanej jeszcze frazy (do „cofnij to”).
    history_ctx: Vec<(u64, TextContext)>,
    history: Vec<Typed>,
    next_id: u64,
    events: Vec<DictationEvent>,
    typed_total: usize,
}

impl DictationMachine {
    /// Automat z konfiguracją.
    pub fn new(cfg: DictationCfg) -> Self {
        Self {
            ctx: TextContext::start(cfg.capitalize_start),
            cfg,
            phase: DictationPhase::Idle,
            target: None,
            mode: DictationMode::Toggle,
            pending: VecDeque::new(),
            history_ctx: Vec::new(),
            history: Vec::new(),
            next_id: 0,
            events: Vec::new(),
            typed_total: 0,
        }
    }

    /// Konfiguracja.
    pub fn cfg(&self) -> &DictationCfg {
        &self.cfg
    }

    /// Cel sesji.
    pub fn target(&self) -> Option<&DictationTarget> {
        self.target.as_ref()
    }

    /// Stan.
    pub fn status(&self) -> DictationStatus {
        DictationStatus {
            phase: self.phase,
            mode: self.mode,
            app: self.target.as_ref().map(|t| t.app.clone()),
            pending_chars: self.pending.iter().map(|(_, t)| t.chars().count()).sum(),
            typed_chars: self.typed_total,
            can_undo: !self.history.is_empty() || !self.pending.is_empty(),
        }
    }

    /// Zdarzenia od ostatniego odczytu.
    pub fn take_events(&mut self) -> Vec<DictationEvent> {
        std::mem::take(&mut self.events)
    }

    /// Zgłasza odmowę (usługa sprawdziła cel).
    pub fn refuse(&mut self, reason: crate::RefuseReason) {
        self.events.push(DictationEvent::Refused { reason });
    }

    /// Start sesji z celem (już sprawdzonym przez usługę: nie chroniony, nie podniesiony).
    pub fn start(&mut self, target: DictationTarget, mode: DictationMode) {
        self.stop(StopReason::Restarted);
        self.events.push(DictationEvent::Started {
            app: target.app.clone(),
            mode,
        });
        self.target = Some(target);
        self.mode = mode;
        self.phase = DictationPhase::Active;
        self.ctx = TextContext::start(self.cfg.capitalize_start);
    }

    /// Koniec sesji (tekst niewpisany przepada — nie trafia do innego okna).
    pub fn stop(&mut self, reason: StopReason) {
        if self.phase == DictationPhase::Idle {
            return;
        }
        let dropped: usize = self.pending.iter().map(|(_, t)| t.chars().count()).sum();
        self.pending.clear();
        self.history_ctx.clear();
        self.history.clear();
        self.target = None;
        self.phase = DictationPhase::Idle;
        self.events.push(DictationEvent::Stopped {
            reason,
            dropped_chars: dropped as u32,
        });
    }

    fn pause(&mut self, reason: PauseReason) {
        if self.phase == DictationPhase::Active {
            self.phase = DictationPhase::Paused(reason);
            // Po zmianie okna albo wejściu użytkownika kursor mógł się przesunąć — nie cofamy.
            self.history.clear();
            self.events.push(DictationEvent::Paused { reason });
        }
    }

    /// Okno na pierwszym planie (obserwacja co krok). `None` — pulpit/ekran blokady.
    pub fn foreground(
        &mut self,
        window: Option<platform_contract::WindowId>,
    ) -> Vec<DictationAction> {
        let Some(target) = &self.target else {
            return Vec::new();
        };
        let on_target = window == Some(target.window);
        match self.phase {
            DictationPhase::Active if !on_target => self.pause(PauseReason::FocusChanged),
            DictationPhase::Paused(PauseReason::FocusChanged) if on_target => {
                self.phase = DictationPhase::Active;
                self.events.push(DictationEvent::Resumed);
            }
            _ => {}
        }
        self.drain()
    }

    /// Użytkownik pisze / wejście zablokowane chwilowo — ponów później (bez zmiany kolejki).
    pub fn user_busy(&mut self) {
        self.pause(PauseReason::UserTyping);
    }

    /// Wznowienie po przerwie użytkownika (np. upłynęło `idle` bez wejścia).
    pub fn resume_after_user(&mut self) -> Vec<DictationAction> {
        if self.phase == DictationPhase::Paused(PauseReason::UserTyping) {
            self.phase = DictationPhase::Active;
            self.events.push(DictationEvent::Resumed);
        }
        self.drain()
    }

    /// Final STT: komenda sterująca albo fraza do wpisania.
    pub fn final_text(
        &mut self,
        text: &str,
        now_ms: u64,
    ) -> Result<Vec<DictationAction>, crate::DictationError> {
        if self.phase == DictationPhase::Idle {
            return Err(crate::DictationError::NotActive);
        }
        match control_command(text) {
            Some(crate::ControlCommand::Undo) => return Ok(self.undo(now_ms)),
            Some(crate::ControlCommand::Stop) => {
                self.stop(StopReason::Voice);
                return Ok(Vec::new());
            }
            None => {}
        }
        if text.chars().count() > self.cfg.max_phrase_chars {
            return Err(crate::DictationError::Rejected(format!(
                "fraza dłuższa niż {} znaków",
                self.cfg.max_phrase_chars
            )));
        }
        let terminal = self.target.as_ref().is_some_and(|t| t.terminal);
        let before = self.ctx;
        let mut out = normalize(text, &mut self.ctx);
        if terminal && self.cfg.block_enter_in_terminals && out.contains('\n') {
            // Enter w terminalu wykonałby polecenie — wpisujemy spację; Enter naciska użytkownik.
            out = out.replace('\n', " ");
            self.events.push(DictationEvent::NewlineBlocked);
        }
        if out.trim().is_empty() {
            self.ctx = before;
            return Ok(Vec::new());
        }
        self.next_id += 1;
        self.pending.push_back((self.next_id, out));
        self.history_ctx.push((self.next_id, before));
        Ok(self.drain())
    }

    fn drain(&mut self) -> Vec<DictationAction> {
        if self.phase != DictationPhase::Active {
            return Vec::new();
        }
        self.pending
            .iter()
            .map(|(id, text)| DictationAction::Type {
                id: *id,
                text: text.clone(),
            })
            .collect()
    }

    /// Wynik wpisywania frazy `id`: `typed` znaków wpisanych (od początku frazy), reszta czeka.
    pub fn typed(&mut self, id: u64, typed: usize, now_ms: u64) {
        let Some(pos) = self.pending.iter().position(|(p, _)| *p == id) else {
            return;
        };
        let text = self.pending[pos].1.clone();
        let total = text.chars().count();
        let typed = typed.min(total);
        if typed > 0 {
            let ctx_before = self
                .history_ctx
                .iter()
                .find(|(p, _)| *p == id)
                .map_or(self.ctx, |(_, c)| *c);
            match self.history.last_mut().filter(|h| h.id == id) {
                Some(h) => h.chars += typed,
                None => self.history.push(Typed {
                    id,
                    chars: typed,
                    at_ms: now_ms,
                    ctx_before,
                }),
            }
            self.typed_total += typed;
            self.events.push(DictationEvent::Typed {
                chars: typed as u32,
            });
        }
        if typed == total {
            self.pending.remove(pos);
            self.history_ctx.retain(|(p, _)| *p != id);
        } else {
            self.pending[pos].1 = text.chars().skip(typed).collect();
        }
        while self.history.len() > 8 {
            self.history.remove(0);
        }
    }

    /// „cofnij to”: porzuca niewpisaną ostatnią frazę albo zleca usunięcie wpisanej.
    pub fn undo(&mut self, now_ms: u64) -> Vec<DictationAction> {
        if let Some((id, _)) = self.pending.pop_back() {
            // Ostatnia fraza nie została jeszcze wpisana — wystarczy ją porzucić.
            if let Some((_, ctx)) = self.history_ctx.iter().find(|(p, _)| *p == id) {
                self.ctx = *ctx;
            }
            self.history_ctx.retain(|(p, _)| *p != id);
            self.events.push(DictationEvent::Undone { chars: 0 });
            return self.drain();
        }
        let Some(last) = self.history.last().cloned() else {
            self.events.push(DictationEvent::UndoUnavailable);
            return Vec::new();
        };
        if self.phase != DictationPhase::Active
            || now_ms.saturating_sub(last.at_ms) > self.cfg.undo_window_ms
        {
            self.events.push(DictationEvent::UndoUnavailable);
            return Vec::new();
        }
        vec![DictationAction::Erase {
            id: last.id,
            chars: last.chars,
        }]
    }

    /// Wynik „cofnij to”: usunięto `chars` znaków frazy `id`.
    pub fn erased(&mut self, id: u64, chars: usize) {
        if let Some(pos) = self.history.iter().position(|h| h.id == id) {
            let h = self.history.remove(pos);
            if chars >= h.chars {
                self.ctx = h.ctx_before;
            }
            self.typed_total = self.typed_total.saturating_sub(chars);
            self.events.push(DictationEvent::Undone {
                chars: chars as u32,
            });
        }
    }
}
