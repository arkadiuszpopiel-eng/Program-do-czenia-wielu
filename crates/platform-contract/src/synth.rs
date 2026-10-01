//! Wejście syntetyczne (F5 v1.5 / F6 v2, PLAN §7.2–7.4): tekst Unicode, skróty, kliknięcia
//! i przewijanie w oknie docelowym. Logika bezpieczeństwa jest tu ([`execute`]), żeby atrapa
//! i implementacja Windows nie mogły się rozjechać:
//!
//! - plan dzieli się na **atomowe paczki** (jedno `SendInput` = cały skrót / klik / ≤ N znaków),
//!   więc przerwanie nigdy nie zostawia wciśniętego modyfikatora ani przycisku;
//! - **przed każdą paczką**: anulowanie, fizyczne wejście użytkownika od startu (hook — wejście
//!   niewstrzyknięte; użytkownik ma pierwszeństwo, §7.4), okno docelowe (fokus dla klawiatury,
//!   okno pod punktem dla myszy) = okno z planu, proces okna nie jest chroniony
//!   ([`TargetGuard`]: Alfa/Broker), proces nie jest podniesiony (UIPI);
//! - limit tempa: odstęp między paczkami, limity długości tekstu i liczby kroków.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::gui::{GuiError, TargetGuard};
use crate::keys::KeyChord;
use crate::synth_plan::plan_batches;
use crate::window::WindowId;

/// Przycisk myszy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    /// Lewy.
    Left,
    /// Prawy.
    Right,
    /// Środkowy.
    Middle,
}

/// Krok planu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum InputStep {
    /// Tekst Unicode (`\n` = Enter, `\t` = Tab; inne znaki sterujące odrzucane).
    Text {
        /// Tekst.
        text: String,
    },
    /// Skrót klawiszowy.
    Keys {
        /// Skrót.
        chord: KeyChord,
    },
    /// Kliknięcie w punkcie ekranu.
    Click {
        /// X (piksele fizyczne).
        x: i32,
        /// Y.
        y: i32,
        /// Przycisk.
        button: MouseButton,
        /// Podwójne.
        double: bool,
    },
    /// Przewinięcie kółkiem w punkcie ekranu (`notches` > 0 = w górę/w prawo).
    Scroll {
        /// X.
        x: i32,
        /// Y.
        y: i32,
        /// Liczba ząbków (±1–±20).
        notches: i32,
        /// Poziomo.
        horizontal: bool,
    },
}

/// Plan wejścia do jednego okna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputPlan {
    /// Okno docelowe.
    pub window: WindowId,
    /// Kroki.
    pub steps: Vec<InputStep>,
}

/// Zdarzenie niskiego poziomu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RawInput {
    /// Klawisz wirtualny.
    Key {
        /// `VK_*`.
        vk: u16,
        /// Puszczenie.
        up: bool,
    },
    /// Jednostka UTF-16 (`KEYEVENTF_UNICODE`).
    Unicode {
        /// Jednostka.
        unit: u16,
        /// Puszczenie.
        up: bool,
    },
    /// Ruch kursora (bezwzględnie).
    MoveTo {
        /// X.
        x: i32,
        /// Y.
        y: i32,
    },
    /// Przycisk myszy.
    Button {
        /// Przycisk.
        button: MouseButton,
        /// Puszczenie.
        up: bool,
    },
    /// Kółko (wielokrotność 120).
    Wheel {
        /// Delta.
        delta: i32,
        /// Poziomo.
        horizontal: bool,
    },
}

/// Gdzie trafia paczka: do okna z fokusem (klawiatura) albo okna pod punktem (mysz).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Aim {
    /// Okno na pierwszym planie.
    Focus,
    /// Okno pod punktem.
    Point {
        /// X.
        x: i32,
        /// Y.
        y: i32,
    },
}

/// Atomowa paczka zdarzeń (jedno `SendInput`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputBatch {
    /// Cel.
    pub aim: Aim,
    /// Zdarzenia (każde wciśnięcie ma puszczenie w tej samej paczce).
    pub events: Vec<RawInput>,
}

/// Tempo i limity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputPacing {
    /// Odstęp między paczkami (ms).
    pub batch_interval_ms: u64,
    /// Ile ms bez fizycznego wejścia przed startem (użytkownik pracuje → `UserActive`).
    pub idle_before_ms: u64,
    /// Jednostek UTF-16 tekstu na paczkę.
    pub text_batch_units: usize,
    /// Maksymalna długość tekstu w planie (znaki).
    pub max_text_chars: usize,
    /// Maksymalna liczba kroków.
    pub max_steps: usize,
}

impl Default for InputPacing {
    fn default() -> Self {
        Self {
            batch_interval_ms: 20,
            idle_before_ms: 800,
            text_batch_units: 16,
            max_text_chars: 5_000,
            max_steps: 32,
        }
    }
}

/// Okno faktycznie trafione przez paczkę.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetWindow {
    /// Okno najwyższego poziomu.
    pub id: WindowId,
    /// PID właściciela.
    pub pid: u32,
    /// Obraz procesu (pusty = nieznany → chroniony).
    pub image: String,
    /// Proces podniesiony.
    pub elevated: bool,
}

/// Raport wysłania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputReport {
    /// Wysłane paczki.
    pub batches: u32,
    /// Wysłane zdarzenia.
    pub events: u32,
    /// Czas (ms).
    pub elapsed_ms: u64,
}

/// Anulowanie wysyłania (kill-switch, „stop”).
#[derive(Debug, Clone, Default)]
pub struct InputControl {
    cancel: Arc<AtomicBool>,
}

impl InputControl {
    /// Nowe sterowanie.
    pub fn new() -> Self {
        Self::default()
    }
    /// Anuluje (przed najbliższą paczką).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    /// Czy anulowano.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// Dostęp niskiego poziomu (Windows: `SendInput` + hook; atrapa: wirtualny pulpit).
pub trait InputBackend: Send + Sync {
    /// Zegar (ms; ta sama domena co [`InputBackend::last_physical_input_ms`]).
    fn now_ms(&self) -> u64;
    /// Pauza (tempo).
    fn pause_ms(&self, ms: u64);
    /// Chwila ostatniego **fizycznego** (niewstrzykniętego) wejścia użytkownika.
    fn last_physical_input_ms(&self) -> Option<u64>;
    /// Okno na pierwszym planie.
    fn foreground_target(&self) -> Option<TargetWindow>;
    /// Okno najwyższego poziomu pod punktem.
    fn target_at(&self, x: i32, y: i32) -> Option<TargetWindow>;
    /// Wstrzykuje paczkę atomowo (wszystkie zdarzenia albo błąd).
    fn inject(&self, events: &[RawInput]) -> Result<(), GuiError>;
}

/// Port wejścia syntetycznego.
pub trait InputPort: Send + Sync {
    /// Wysyła plan ([`execute`] z kontrolami przed każdą paczką).
    fn send(&self, plan: &InputPlan, control: &InputControl) -> Result<InputReport, GuiError>;
}

/// Wykonuje plan z kontrolami przed każdą paczką (opis modułu). Zwraca błąd przy pierwszym
/// naruszeniu — nic więcej nie jest wysyłane.
pub fn execute(
    plan: &InputPlan,
    backend: &dyn InputBackend,
    guard: &TargetGuard,
    pacing: &InputPacing,
    control: &InputControl,
) -> Result<InputReport, GuiError> {
    let batches = plan_batches(plan, pacing)?;
    let started = backend.now_ms();
    if backend
        .last_physical_input_ms()
        .is_some_and(|t| t >= started.saturating_sub(pacing.idle_before_ms))
    {
        return Err(GuiError::UserActive);
    }
    let mut report = InputReport {
        batches: 0,
        events: 0,
        elapsed_ms: 0,
    };
    for (i, batch) in batches.iter().enumerate() {
        if control.is_cancelled() {
            return Err(GuiError::Cancelled);
        }
        if backend
            .last_physical_input_ms()
            .is_some_and(|t| t >= started)
        {
            return Err(GuiError::UserInterrupted {
                sent: report.batches,
            });
        }
        let target = match batch.aim {
            Aim::Focus => backend.foreground_target(),
            Aim::Point { x, y } => backend.target_at(x, y),
        };
        let Some(target) = target else {
            return Err(GuiError::TargetChanged {
                expected: plan.window.0,
                actual: None,
            });
        };
        guard.check(target.pid, &target.image, "wejście syntetyczne")?;
        if target.id != plan.window {
            return Err(GuiError::TargetChanged {
                expected: plan.window.0,
                actual: Some(target.id.0),
            });
        }
        if target.elevated {
            return Err(GuiError::Elevated);
        }
        backend.inject(&batch.events)?;
        report.batches += 1;
        report.events += u32::try_from(batch.events.len()).unwrap_or(u32::MAX);
        if i + 1 < batches.len() {
            backend.pause_ms(pacing.batch_interval_ms);
        }
    }
    report.elapsed_ms = backend.now_ms().saturating_sub(started);
    Ok(report)
}

#[cfg(test)]
#[path = "synth_tests.rs"]
mod tests;
