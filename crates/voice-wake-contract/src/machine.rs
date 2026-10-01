//! Deterministyczny automat aktywacji (wspólny dla `-impl` i `-fake`): PTT (wciśnięcie/puszczenie
//! z hooka `WH_KEYBOARD_LL`), przełącznik, przycisk w UI, wyciszenie, „nie przeszkadzać”, stan
//! mikrofonu, adresowanie po imieniu z transkryptu.

use personas_contract::{Cast, Persona, PersonaId, parse_addressee, resolve_addressee};
use platform_contract::HotkeyId;

use crate::{MicState, WakeEvent, WakeInput, WakeSource};

/// Automat aktywacji słuchania.
#[derive(Debug, Clone)]
pub struct WakeMachine {
    ptt: Option<HotkeyId>,
    toggle: Option<HotkeyId>,
    name_addressing: bool,
    personas: Vec<Persona>,
    cast: Option<Cast>,
    listening: Option<WakeSource>,
    ptt_down: bool,
    hearing: bool,
    processing: bool,
    muted: bool,
    dnd: bool,
    elevated: bool,
    mic: MicState,
    addressed: Option<PersonaId>,
}

impl WakeMachine {
    /// Automat z personami (imiona do adresowania) i obsadą (Dyrygentka = adresatka domyślna).
    pub fn new(personas: Vec<Persona>, cast: Option<Cast>) -> Self {
        Self {
            ptt: None,
            toggle: None,
            name_addressing: true,
            personas,
            cast,
            listening: None,
            ptt_down: false,
            hearing: false,
            processing: false,
            muted: false,
            dnd: false,
            elevated: false,
            mic: MicState::Off,
            addressed: None,
        }
    }

    /// Ustawia identyfikatory zarejestrowanych skrótów (PTT, przełącznik).
    pub fn set_keys(&mut self, ptt: Option<HotkeyId>, toggle: Option<HotkeyId>) {
        self.ptt = ptt;
        self.toggle = toggle;
    }

    /// Adresowanie po imieniu z transkryptu.
    pub fn set_name_addressing(&mut self, on: bool) {
        self.name_addressing = on;
    }

    /// Zmienia obsadę (Dyrygentka).
    pub fn set_cast(&mut self, cast: Option<Cast>) {
        self.cast = cast;
    }

    /// Stan mikrofonu.
    pub fn mic_state(&self) -> MicState {
        self.mic
    }

    /// Źródło bieżącego słuchania.
    pub fn listening(&self) -> Option<WakeSource> {
        self.listening
    }

    /// Tryb „nie przeszkadzać”.
    pub fn dnd(&self) -> bool {
        self.dnd
    }

    /// Ostatnia adresatka (z ostatniego transkryptu).
    pub fn last_addressed(&self) -> Option<&PersonaId> {
        self.addressed.as_ref()
    }

    /// Adresatka wypowiedzi: imię wygrywa, inaczej Dyrygentka obsady (`None` bez obsady).
    pub fn addressee(&self, text: &str) -> Option<PersonaId> {
        match &self.cast {
            Some(cast) => resolve_addressee(text, &self.personas, cast),
            None => parse_addressee(text, &self.personas),
        }
    }

    fn target_mic(&self) -> MicState {
        if self.muted {
            MicState::Muted
        } else if self.listening.is_some() {
            if self.hearing {
                MicState::Hearing
            } else {
                MicState::Listening
            }
        } else if self.processing {
            MicState::Processing
        } else {
            MicState::Off
        }
    }

    fn sync_mic(&mut self, out: &mut Vec<WakeEvent>) {
        let m = self.target_mic();
        if m != self.mic {
            self.mic = m;
            out.push(WakeEvent::MicState { state: m });
        }
    }

    fn start(&mut self, source: WakeSource, out: &mut Vec<WakeEvent>) {
        if self.muted || self.listening.is_some() {
            return;
        }
        self.listening = Some(source);
        out.push(WakeEvent::ListenStart {
            addressed: None,
            source,
        });
    }

    fn stop(&mut self, out: &mut Vec<WakeEvent>) {
        if let Some(source) = self.listening.take() {
            self.hearing = false;
            out.push(WakeEvent::ListenStop { source });
        }
    }

    /// Przetwarza wejście; zwraca zdarzenia (w kolejności).
    pub fn handle(&mut self, input: WakeInput) -> Vec<WakeEvent> {
        let mut out = Vec::new();
        match input {
            WakeInput::Key { id, pressed } if Some(id) == self.ptt => {
                self.ptt_edge(WakeSource::Ptt, pressed, &mut out)
            }
            WakeInput::Key { id, pressed: true } if Some(id) == self.toggle => {
                self.toggle(WakeSource::Toggle, &mut out)
            }
            WakeInput::Key { .. } => {}
            WakeInput::UiPtt { pressed } => self.ptt_edge(WakeSource::Ui, pressed, &mut out),
            WakeInput::UiToggle => self.toggle(WakeSource::Ui, &mut out),
            WakeInput::Vad { speech } => {
                self.hearing = speech && self.listening.is_some();
            }
            WakeInput::Processing { busy } => self.processing = busy,
            WakeInput::Transcript { text } => {
                if self.name_addressing {
                    let explicit = parse_addressee(&text, &self.personas);
                    let who = explicit.clone().or_else(|| self.addressee(&text));
                    if let Some(persona) = who {
                        self.addressed = Some(persona.clone());
                        out.push(WakeEvent::Addressed {
                            persona,
                            by_name: explicit.is_some(),
                        });
                    }
                }
            }
            WakeInput::SetMuted { muted } => {
                self.muted = muted;
                if muted {
                    self.ptt_down = false;
                    self.stop(&mut out);
                }
            }
            WakeInput::SetDnd { on } => {
                // DND wycisza słowa wywoławcze i mowę proaktywną, nie PTT (SPEC).
                if on != self.dnd {
                    self.dnd = on;
                    out.push(WakeEvent::Dnd { on });
                }
            }
            WakeInput::ElevatedForeground { elevated } => {
                if elevated && !self.elevated {
                    out.push(WakeEvent::BlockedElevatedForeground);
                }
                self.elevated = elevated;
            }
        }
        self.sync_mic(&mut out);
        out
    }

    fn ptt_edge(&mut self, source: WakeSource, pressed: bool, out: &mut Vec<WakeEvent>) {
        if pressed {
            if !self.ptt_down && !self.muted {
                self.ptt_down = true;
                self.start(source, out);
            }
        } else if self.ptt_down {
            self.ptt_down = false;
            if self.listening == Some(source) {
                self.stop(out);
            }
        }
    }

    fn toggle(&mut self, source: WakeSource, out: &mut Vec<WakeEvent>) {
        if self.listening.is_some() {
            self.stop(out);
        } else {
            self.start(source, out);
        }
    }
}
