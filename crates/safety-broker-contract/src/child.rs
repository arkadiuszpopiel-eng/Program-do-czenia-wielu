//! Profil Dziecko — kłódka trybów pracy i PIN rodzica (plan 50 §4.4, decyzja właściciela 3.11):
//! bez PIN-u obowiązuje najwyżej „Ręcznie” (L1, z „Planem” L0); wyższe tryby, „Max dostęp”, zgody
//! stałe i przełączenie z okna zgody są zamknięte. Kłódkę sprawdza Sędzia (sufit w
//! [`crate::AutonomyTable`]), nie tylko interfejs. PIN przechodzi przez potok tylko w żądaniu
//! zmiany; nigdy nie trafia do Audytu, dziennika ani odpowiedzi.

use risk_classifier_contract::AutonomyLevel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Najwyższy tryb dziecka przy zamkniętej kłódce.
pub const CHILD_CEILING: AutonomyLevel = AutonomyLevel::L1;
/// Najkrótszy PIN (cyfry).
pub const PIN_MIN_DIGITS: usize = 4;
/// Najdłuższy PIN (cyfry).
pub const PIN_MAX_DIGITS: usize = 8;
/// Błędne próby przed blokadą.
pub const PIN_MAX_FAILURES: u32 = 5;
/// Pierwsza blokada po serii błędnych prób (15 min); każda następna dwa razy dłuższa.
pub const PIN_BLOCK_MS: u64 = 15 * 60_000;
/// Otwarcie „do zamknięcia Alfy” — bez terminu, do restartu Sędziego.
pub const OPEN_UNTIL_CLOSE: u64 = u64::MAX;

/// Sufit poziomu w profilu Dziecko; `open_until_ms` — kłódka otwarta PIN-em rodzica do tej chwili.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChildCeiling {
    /// Najwyższy poziom przy zamkniętej kłódce.
    pub level: AutonomyLevel,
    /// Kłódka otwarta do (ms); `None` — zamknięta.
    pub open_until_ms: Option<u64>,
}

impl ChildCeiling {
    /// Zamknięta kłódka z sufitem [`CHILD_CEILING`].
    #[must_use]
    pub const fn locked() -> Self {
        Self {
            level: CHILD_CEILING,
            open_until_ms: None,
        }
    }

    /// Czy kłódka jest teraz zamknięta.
    #[must_use]
    pub fn is_locked(&self, now_ms: u64) -> bool {
        self.open_until_ms.is_none_or(|until| now_ms >= until)
    }
}

/// PIN rodzica (tylko cyfry). `Debug` nie pokazuje wartości; pamięć zerowana przy zwolnieniu.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Pin(String);

impl Pin {
    /// PIN z tekstu: 4–8 cyfr ASCII.
    pub fn new(digits: &str) -> Result<Self, String> {
        let ok = (PIN_MIN_DIGITS..=PIN_MAX_DIGITS).contains(&digits.len())
            && digits.bytes().all(|b| b.is_ascii_digit());
        if ok {
            Ok(Self(digits.to_owned()))
        } else {
            Err(format!(
                "PIN ma od {PIN_MIN_DIGITS} do {PIN_MAX_DIGITS} cyfr"
            ))
        }
    }

    /// Cyfry (tylko do wyliczenia skrótu w Sędzim).
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Czy format jest poprawny (PIN z potoku nie przeszedł przez [`Pin::new`]).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        Self::new(&self.0).is_ok()
    }
}

impl Drop for Pin {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}

impl std::fmt::Debug for Pin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Pin(***)")
    }
}

/// Zmiana profilu Dziecko (tylko rola Jądra — interfejs właściciela).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ChildLockChange {
    /// Ustawia PIN (pierwszy raz `old` = `None`; zmiana wymaga starego PIN-u).
    SetPin {
        /// Dotychczasowy PIN.
        old: Option<Pin>,
        /// Nowy PIN.
        new: Pin,
    },
    /// Włącza albo wyłącza profil Dziecko (PIN rodzica).
    Enable {
        /// PIN rodzica.
        pin: Pin,
        /// `true` — profil Dziecko włączony.
        on: bool,
    },
    /// Otwiera kłódkę na czas liczony zegarem Sędziego (PIN rodzica).
    Open {
        /// PIN rodzica.
        pin: Pin,
        /// Na ile (ms); `None` — do zamknięcia Alfy (restartu Sędziego).
        for_ms: Option<u64>,
    },
    /// Zamyka kłódkę od razu (bez PIN-u — zawężenie może każdy).
    Close,
    /// Sprawdza PIN bez zmiany stanu (dziennik rodzica, plan 50 Z.7) — z tym samym limitem prób.
    Verify {
        /// PIN rodzica.
        pin: Pin,
    },
}

/// Stan profilu Dziecko dla interfejsu (bez PIN-u i skrótu).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChildLockView {
    /// Czy rodzic ustawił PIN.
    pub pin_set: bool,
    /// Czy profil Dziecko jest włączony.
    pub enabled: bool,
    /// Kłódka otwarta do (ms); `None` — zamknięta albo profil wyłączony.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_until_ms: Option<u64>,
    /// Blokada po błędnych próbach do (ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_until_ms: Option<u64>,
    /// Pozostałe próby przed blokadą.
    pub attempts_left: u32,
    /// Zapis kłódki poza zasięgiem konta dziecka (Sędzia jako usługa na osobnym koncie); `false` —
    /// ochrona umowna (dziecko na tym samym koncie mogłoby usunąć zapis).
    #[serde(default)]
    pub tamper_resistant: bool,
    /// Stanu nie da się odczytać (zerwane łącze z Sędzią) — interfejs pokazuje „nieznany” zamiast
    /// „bez PIN-u”; samą kłódkę i tak sprawdza Sędzia przy każdym działaniu.
    #[serde(default)]
    pub unknown: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_format_and_redacted_debug() {
        assert!(Pin::new("1234").is_ok());
        assert!(Pin::new("12345678").is_ok());
        for bad in ["123", "123456789", "12a4", "", "１２３４", " 1234"] {
            assert!(Pin::new(bad).is_err(), "{bad:?}");
        }
        let pin = Pin::new("9876").unwrap();
        assert_eq!(format!("{pin:?}"), "Pin(***)");
        let forged: Pin = serde_json::from_str("\"12\"").unwrap();
        assert!(!forged.is_valid());
    }

    #[test]
    fn ceiling_locks_until_the_deadline() {
        let mut c = ChildCeiling::locked();
        assert!(c.is_locked(0));
        c.open_until_ms = Some(1_000);
        assert!(!c.is_locked(999));
        assert!(c.is_locked(1_000));
        c.open_until_ms = Some(OPEN_UNTIL_CLOSE);
        assert!(!c.is_locked(u64::MAX - 1));
    }
}
