//! Komendy głosowe szybkiej ścieżki.

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Komenda głosowa (bez akcji destrukcyjnych — SPEC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum VoiceCommand {
    /// „stop”, „stój” — stop mowy (nie zabija pracy w tle).
    Stop,
    /// „czekaj”, „poczekaj” — wstrzymanie mowy z możliwością wznowienia.
    Wait,
    /// „pauza” — jak `Wait`.
    Pause,
    /// „wznów”, „kontynuuj”, „mów dalej” — wznowienie od punktu cięcia.
    Resume,
    /// „powtórz”.
    Repeat,
    /// „anuluj” — stop mowy i bieżącego zadania.
    Cancel,
    /// „głośniej”.
    VolumeUp,
    /// „ciszej”.
    VolumeDown,
    /// „wycisz mikrofon”.
    MuteMic,
    /// „przełącz na Deltę”.
    SwitchPersona {
        /// Docelowa persona.
        persona: PersonaId,
    },
    /// „nie przeszkadzać”.
    DoNotDisturb,
    /// „stop wszystko” — kill-switch (zatrzymuje, więc bezpieczny; wykonywany natychmiast).
    StopAll,
    /// Samodzielne „nie” z pauzą przed i po, tylko w `Speaking` — przerwanie.
    No,
}

/// Rodzaj komendy (bez danych) — klucz reguł gramatyki.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    /// Zob. [`VoiceCommand::Stop`].
    Stop,
    /// Zob. [`VoiceCommand::Wait`].
    Wait,
    /// Zob. [`VoiceCommand::Pause`].
    Pause,
    /// Zob. [`VoiceCommand::Resume`].
    Resume,
    /// Zob. [`VoiceCommand::Repeat`].
    Repeat,
    /// Zob. [`VoiceCommand::Cancel`].
    Cancel,
    /// Zob. [`VoiceCommand::VolumeUp`].
    VolumeUp,
    /// Zob. [`VoiceCommand::VolumeDown`].
    VolumeDown,
    /// Zob. [`VoiceCommand::MuteMic`].
    MuteMic,
    /// Zob. [`VoiceCommand::SwitchPersona`] (wymaga slotu `{persona}`).
    SwitchPersona,
    /// Zob. [`VoiceCommand::DoNotDisturb`].
    DoNotDisturb,
    /// Zob. [`VoiceCommand::StopAll`].
    StopAll,
    /// Zob. [`VoiceCommand::No`].
    No,
}

/// Jak wykonać komendę.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommandSafety {
    /// Natychmiast, bez potwierdzenia (komendy zatrzymujące lub kosmetyczne).
    Immediate,
    /// Tylko przez Brokera z potwierdzeniem fizycznym (zmiana uprawnień — brak takich w v0).
    NeedsBroker,
}

impl VoiceCommand {
    /// Rodzaj komendy.
    pub fn kind(&self) -> CommandKind {
        match self {
            Self::Stop => CommandKind::Stop,
            Self::Wait => CommandKind::Wait,
            Self::Pause => CommandKind::Pause,
            Self::Resume => CommandKind::Resume,
            Self::Repeat => CommandKind::Repeat,
            Self::Cancel => CommandKind::Cancel,
            Self::VolumeUp => CommandKind::VolumeUp,
            Self::VolumeDown => CommandKind::VolumeDown,
            Self::MuteMic => CommandKind::MuteMic,
            Self::SwitchPersona { .. } => CommandKind::SwitchPersona,
            Self::DoNotDisturb => CommandKind::DoNotDisturb,
            Self::StopAll => CommandKind::StopAll,
            Self::No => CommandKind::No,
        }
    }

    /// Komenda przerywająca mowę agentki (działa bez adresowania, gdy agentka mówi/myśli).
    pub fn is_barge_in(&self) -> bool {
        matches!(
            self,
            Self::Stop | Self::Wait | Self::Pause | Self::Cancel | Self::StopAll | Self::No
        )
    }

    /// Wszystkie komendy v0 są bezpieczne: zatrzymują, wznawiają albo zmieniają głośność.
    pub fn safety(&self) -> CommandSafety {
        CommandSafety::Immediate
    }

    /// Komenda bez danych dla rodzaju (dla `SwitchPersona` — `None`, bo potrzebna persona).
    pub fn from_kind(kind: CommandKind) -> Option<Self> {
        Some(match kind {
            CommandKind::Stop => Self::Stop,
            CommandKind::Wait => Self::Wait,
            CommandKind::Pause => Self::Pause,
            CommandKind::Resume => Self::Resume,
            CommandKind::Repeat => Self::Repeat,
            CommandKind::Cancel => Self::Cancel,
            CommandKind::VolumeUp => Self::VolumeUp,
            CommandKind::VolumeDown => Self::VolumeDown,
            CommandKind::MuteMic => Self::MuteMic,
            CommandKind::DoNotDisturb => Self::DoNotDisturb,
            CommandKind::StopAll => Self::StopAll,
            CommandKind::No => Self::No,
            CommandKind::SwitchPersona => return None,
        })
    }
}

/// Co robi agentka w chwili wypowiedzi (z `voice-dialog`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentActivity {
    /// Nie mówi i nie generuje (Idle/Listening/UserSpeaking/Interrupted).
    Silent,
    /// Generuje odpowiedź (Thinking).
    Thinking,
    /// Mówi (Speaking) — jedyny stan, w którym samodzielne „nie” przerywa.
    Speaking,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_and_safety() {
        let all = [
            CommandKind::Stop,
            CommandKind::Wait,
            CommandKind::Pause,
            CommandKind::Resume,
            CommandKind::Repeat,
            CommandKind::Cancel,
            CommandKind::VolumeUp,
            CommandKind::VolumeDown,
            CommandKind::MuteMic,
            CommandKind::DoNotDisturb,
            CommandKind::StopAll,
            CommandKind::No,
        ];
        for kind in all {
            let cmd = VoiceCommand::from_kind(kind).unwrap();
            assert_eq!(cmd.kind(), kind);
            assert_eq!(cmd.safety(), CommandSafety::Immediate);
        }
        let switch = VoiceCommand::SwitchPersona {
            persona: PersonaId::delta(),
        };
        assert_eq!(switch.kind(), CommandKind::SwitchPersona);
        assert!(VoiceCommand::from_kind(CommandKind::SwitchPersona).is_none());
        assert!(VoiceCommand::StopAll.is_barge_in() && !VoiceCommand::VolumeUp.is_barge_in());
        let json = serde_json::to_value(&switch).unwrap();
        assert_eq!(json["command"], "switch_persona");
        assert_eq!(json["persona"], "delta");
    }
}
