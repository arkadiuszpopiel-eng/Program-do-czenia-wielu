//! Tabela reguł (polityka Jądra): identyfikatory, zasięg poziomów, opisy i progi.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::{AutonomyLevel, SttConfidence};

/// Identyfikator reguły decyzyjnej.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleId {
    /// Twarda blokada Jądra (każdy poziom).
    KernelBlock,
    /// Destrukcja zlecona głosem → potwierdzenie nie-głosem (każdy poziom, §6.10).
    VoiceDestructive,
    /// Zmiana stanu zlecona głosem przy niskiej pewności STT (każdy poziom).
    VoiceLowConfidence,
    /// Ryzykowna akcja głosem bez weryfikacji mówcy (każdy poziom, do F5).
    VoiceUnverifiedRisky,
    /// Operacja administracyjna → zgoda per eskalacja (§8.4, każdy poziom).
    AdminConsent,
    /// Dane prywatne + niezaufana treść + kanał wyjścia (każdy poziom).
    Trifecta,
    /// Egress z sesji `tainted` (każdy poziom, THREAT_MODEL §8).
    TaintedEgress,
    /// L0/L1: każda zmiana wymaga zgody.
    MutationNeedsYes,
    /// L2: usuwanie, egress, instalacja, sekrety, nieodwracalne.
    RiskyAtL2,
    /// Sesja `tainted` i wysokie ryzyko (≤ L3).
    TaintedHighRisk,
    /// Akcja na podstawie niezaufanej treści (≤ L3).
    UntrustedSource,
    /// Nieodwracalna poza zakresem (≤ L3).
    IrreversibleOutside,
    /// Ryzyko krytyczne (≤ L3).
    CriticalRisk,
    /// Host spoza egress-allowlisty (≤ L3).
    EgressNotAllowlisted,
    /// `gui.control` poza wskazanymi aplikacjami (≤ L3).
    GuiOutsideApps,
}

/// Opis reguły (UI „dlaczego pyta”, audyt).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleDescription {
    /// Identyfikator.
    pub id: RuleId,
    /// Najwyższy poziom, na którym reguła działa (`None` = każdy poziom, także L4).
    pub applies_up_to: Option<AutonomyLevel>,
    /// Czy „zawsze zezwalaj w tym zakresie” może pokryć tę regułę (nigdy dla reguł każdego poziomu).
    pub grantable: bool,
    /// Czy wymaga potwierdzenia nie-głosem.
    pub non_voice: bool,
    /// Opis po polsku.
    pub description_pl: &'static str,
}

impl RuleDescription {
    /// Czy reguła działa na danym poziomie.
    pub fn applies_at(&self, level: AutonomyLevel) -> bool {
        self.applies_up_to.is_none_or(|max| level <= max)
    }
}

const fn rule(
    id: RuleId,
    applies_up_to: Option<AutonomyLevel>,
    grantable: bool,
    non_voice: bool,
    description_pl: &'static str,
) -> RuleDescription {
    RuleDescription {
        id,
        applies_up_to,
        grantable,
        non_voice,
        description_pl,
    }
}

/// Tabela reguł w kolejności oceny. Reguły „każdego poziomu” nigdy nie są `grantable`.
pub const RULES: [RuleDescription; 15] = [
    rule(
        RuleId::KernelBlock,
        None,
        false,
        false,
        "twarda blokada Jądra — obowiązuje na każdym poziomie, także L4",
    ),
    rule(
        RuleId::VoiceDestructive,
        None,
        false,
        true,
        "destrukcja zlecona głosem wymaga potwierdzenia kliknięciem lub klawiszem",
    ),
    rule(
        RuleId::VoiceLowConfidence,
        None,
        false,
        true,
        "niska pewność rozpoznania mowy — potwierdź zmianę kliknięciem lub klawiszem",
    ),
    rule(
        RuleId::VoiceUnverifiedRisky,
        None,
        false,
        true,
        "ryzykowna akcja głosem bez weryfikacji mówcy wymaga potwierdzenia nie-głosem",
    ),
    rule(
        RuleId::AdminConsent,
        None,
        false,
        false,
        "operacja administracyjna wymaga zgody przy każdej eskalacji",
    ),
    rule(
        RuleId::Trifecta,
        None,
        false,
        false,
        "dane prywatne + niezaufana treść + wysyłka na zewnątrz wymagają potwierdzenia",
    ),
    rule(
        RuleId::TaintedEgress,
        None,
        false,
        false,
        "sesja widziała niezaufaną treść — wysyłka na zewnątrz wymaga potwierdzenia",
    ),
    rule(
        RuleId::MutationNeedsYes,
        Some(AutonomyLevel::L1),
        true,
        false,
        "na tym poziomie każda zmiana wymaga Twojego „tak”",
    ),
    rule(
        RuleId::RiskyAtL2,
        Some(AutonomyLevel::L2),
        true,
        false,
        "usuwanie, wysyłka, instalacja, sekrety i akcje nieodwracalne wymagają zgody",
    ),
    rule(
        RuleId::TaintedHighRisk,
        Some(AutonomyLevel::L3),
        false,
        false,
        "sesja widziała niezaufaną treść — akcja wysokiego ryzyka wymaga potwierdzenia",
    ),
    rule(
        RuleId::UntrustedSource,
        Some(AutonomyLevel::L3),
        false,
        false,
        "akcja wynika z niezaufanej treści (strona, mail, plik)",
    ),
    rule(
        RuleId::IrreversibleOutside,
        Some(AutonomyLevel::L3),
        true,
        false,
        "akcja nieodwracalna poza zakresem sesji",
    ),
    rule(
        RuleId::CriticalRisk,
        Some(AutonomyLevel::L3),
        false,
        false,
        "ryzyko krytyczne (np. trwałe usunięcie wielu plików)",
    ),
    rule(
        RuleId::EgressNotAllowlisted,
        Some(AutonomyLevel::L3),
        true,
        false,
        "host spoza egress-allowlisty",
    ),
    rule(
        RuleId::GuiOutsideApps,
        Some(AutonomyLevel::L3),
        true,
        false,
        "sterowanie aplikacją spoza wskazanych",
    ),
];

/// Opis reguły po identyfikatorze.
pub fn describe(id: RuleId) -> RuleDescription {
    RULES
        .iter()
        .copied()
        .find(|r| r.id == id)
        .unwrap_or(RULES[0])
}

/// Progi klasyfikatora (`[security.risk]`, polityka Jądra — zmienia tylko Broker).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct RiskPolicy {
    /// Minimalna pewność STT; niższa podnosi ryzyko i wymaga potwierdzenia zmian.
    pub stt_confidence_min: SttConfidence,
    /// Od tylu obiektów akcja jest „masowa”.
    pub bulk_threshold: u32,
}

impl Default for RiskPolicy {
    fn default() -> Self {
        Self {
            stt_confidence_min: SttConfidence::from_permille(800),
            bulk_threshold: 50,
        }
    }
}

impl RiskPolicy {
    /// Walidacja: próg STT w 500–990‰, próg masowości ≥ 2.
    pub fn validate(&self) -> Result<(), String> {
        let stt = self.stt_confidence_min.permille();
        if !(500..=990).contains(&stt) {
            return Err(format!("stt_confidence_min {stt}‰ poza zakresem 500–990‰"));
        }
        if self.bulk_threshold < 2 {
            return Err("bulk_threshold musi być ≥ 2".into());
        }
        Ok(())
    }
}
