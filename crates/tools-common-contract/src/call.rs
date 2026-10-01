//! Wywołanie narzędzia: kontekst (podmiot, źródło polecenia, anulowanie), wynik dla modelu
//! i UI (status, dane, niezaufana treść, krok „Cofnij”, intencja dla UI) oraz trait [`Tool`].

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use core_bus_contract::RunId;
use risk_classifier_contract::{CommandOrigin, KernelRule};
use safety_broker_contract::{ApprovalId, ApprovalTicket, Holder, TaintSource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::manifest::ToolManifest;

/// Domyślny limit czekania na zatwierdzenie w Broker-UI.
pub const DEFAULT_APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Obserwator wywołania (agent-runtime → zdarzenia `agent.step.*`, karta „czeka na zatwierdzenie”).
pub trait ToolObserver: Send + Sync {
    /// Broker poprosił właściciela o zatwierdzenie (karta w Broker-UI).
    fn approval_requested(&self, ticket: &ApprovalTicket);
    /// Prośba rozstrzygnięta (`approved = false`: odmowa, wygaśnięcie, limit czasu).
    fn approval_resolved(&self, id: ApprovalId, approved: bool);
}

/// Kontekst wywołania narzędzia.
#[derive(Clone)]
pub struct ToolCtx {
    /// Podmiot (sesja, agentka, rola) — ten sam dla `decide` i `verify`.
    pub holder: Holder,
    /// Źródło polecenia przebiegu (tekst/głos właściciela, agentka).
    pub origin: CommandOrigin,
    /// Przebieg.
    pub run: Option<RunId>,
    /// Numer kroku przebiegu.
    pub step: u32,
    /// Etykieta kroku „Cofnij” (np. „Delta: porządki w Pobranych”).
    pub label: String,
    /// Katalog roboczy (ścieżki względne, `cwd` powłoki).
    pub workdir: Option<String>,
    /// Argumenty mogą pochodzić z niezaufanej treści (heurystyka przepływu danych runtime).
    pub untrusted_args: bool,
    /// Anulowanie (kill-switch, „stop”, anulowanie przebiegu).
    pub cancel: CancellationToken,
    /// Limit czekania na zatwierdzenie.
    pub approval_timeout: Duration,
    /// Obserwator (zdarzenia kroków).
    pub observer: Option<Arc<dyn ToolObserver>>,
}

impl std::fmt::Debug for ToolCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolCtx")
            .field("holder", &self.holder)
            .field("origin", &self.origin)
            .field("run", &self.run)
            .field("step", &self.step)
            .field("workdir", &self.workdir)
            .field("untrusted_args", &self.untrusted_args)
            .finish_non_exhaustive()
    }
}

impl ToolCtx {
    /// Kontekst minimalny: sesja + agentka, polecenie tekstowe właściciela.
    pub fn new(holder: Holder) -> Self {
        Self {
            holder,
            origin: CommandOrigin::UserText,
            run: None,
            step: 0,
            label: String::new(),
            workdir: None,
            untrusted_args: false,
            cancel: CancellationToken::new(),
            approval_timeout: DEFAULT_APPROVAL_TIMEOUT,
            observer: None,
        }
    }

    /// Ustawia katalog roboczy (builder).
    #[must_use]
    pub fn with_workdir(mut self, dir: &str) -> Self {
        self.workdir = Some(dir.to_owned());
        self
    }

    /// Etykieta kroku „Cofnij” (domyślnie: „<agentka>: <tytuł narzędzia>”).
    pub fn undo_label(&self, manifest: &ToolManifest) -> String {
        if !self.label.is_empty() {
            return self.label.clone();
        }
        let who = self.holder.agent.as_ref().map_or("Agentka", |a| a.as_str());
        format!("{who}: {}", manifest.title.to_lowercase())
    }
}

/// Dlaczego narzędzie nie wykonało akcji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum DenialReason {
    /// Twarda blokada Jądra (każdy poziom, także L4).
    KernelBlock {
        /// Reguła.
        rule: KernelRule,
    },
    /// Ścieżka/polecenie na deny-liście (sprawdzenie narzędzia przed Brokerem).
    DenyList,
    /// Właściciel odmówił w Broker-UI.
    OwnerDenied {
        /// Prośba.
        approval: ApprovalId,
    },
    /// Prośba wygasła albo unieważniona (kill-switch).
    ApprovalExpired {
        /// Prośba.
        approval: ApprovalId,
    },
    /// Brak decyzji w limicie czasu narzędzia.
    ApprovalTimeout {
        /// Prośba.
        approval: ApprovalId,
    },
    /// Token odrzucony przy weryfikacji (zakres, podmiot, wygaśnięcie).
    TokenRejected,
    /// Audyt niedostępny — Broker nie wydaje tokenów (fail-closed).
    AuditUnavailable,
    /// Polityka narzędzia (np. polecenie sieciowe bez jawnego hosta, zakres za duży na snapshot).
    Policy,
}

impl DenialReason {
    /// Opis po polsku dla modelu i UI.
    pub fn describe(&self) -> String {
        match self {
            Self::KernelBlock { rule } => format!("blokada Jądra — {}", rule.description_pl()),
            Self::DenyList => {
                "ścieżka lub polecenie na deny-liście (poświadczenia, dane Jądra)".into()
            }
            Self::OwnerDenied { .. } => "właściciel odmówił zgody".into(),
            Self::ApprovalExpired { .. } => {
                "prośba o zgodę wygasła albo została unieważniona".into()
            }
            Self::ApprovalTimeout { .. } => "brak decyzji właściciela w wyznaczonym czasie".into(),
            Self::TokenRejected => "Broker odrzucił token zdolności".into(),
            Self::AuditUnavailable => "audyt niedostępny — Broker nie wydaje zgód".into(),
            Self::Policy => "zasady narzędzia nie pozwalają na tę akcję".into(),
        }
    }
}

/// Rodzaj błędu wykonania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorKind {
    /// Argumenty niezgodne ze schematem.
    InvalidArgs,
    /// Nie znaleziono.
    NotFound,
    /// Cel już istnieje.
    AlreadyExists,
    /// Błąd we/wy lub platformy.
    Io,
    /// Nieobsługiwane na tej platformie.
    Unsupported,
    /// Przekroczony limit czasu wykonania.
    Timeout,
    /// Błąd wewnętrzny (Broker, dziennik cofania).
    Internal,
}

/// Status wywołania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ToolStatus {
    /// Wykonano.
    Ok,
    /// Nie wykonano — odmowa (powód czytelny dla modelu).
    Denied {
        /// Powód.
        reason: DenialReason,
    },
    /// Nie wykonano — akcja wymaga potwierdzenia właściciela poza pętlą (intencja dla UI).
    NeedsConfirmation,
    /// Błąd wykonania.
    Failed {
        /// Rodzaj.
        error: ToolErrorKind,
    },
    /// Anulowano.
    Cancelled,
}

/// Usługa, która cofa krok.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UndoService {
    /// `undo-journal` (`fs.*`, snapshot zakresu shella).
    Journal,
    /// `tools-clipboard` (poprzednia zawartość schowka).
    Clipboard,
}

/// Krok „Cofnij” (karta/toast w UI, Replay).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UndoRef {
    /// Usługa.
    pub service: UndoService,
    /// Identyfikator kroku w usłudze (`StepId` dziennika).
    pub id: u64,
    /// Tekst karty („Delta: zapisano 1 plik”).
    pub text: String,
}

/// Intencja dla UI: akcja, którą właściciel wykonuje sam (np. „uruchom w terminalu”,
/// potwierdzenie trwałego usunięcia).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolIntent {
    /// Rodzaj (`shell.open_in_terminal`, `fs.confirm_delete_permanent`).
    pub kind: String,
    /// Tytuł karty (po polsku).
    pub title: String,
    /// Szczegóły (polecenie, ścieżki).
    pub details: serde_json::Value,
}

/// Obraz w wyniku (np. schowek).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToolImage {
    /// Typ MIME.
    pub media_type: String,
    /// Dane base64.
    pub data_base64: String,
}

/// Wynik narzędzia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolOutcome {
    /// Status.
    #[serde(flatten)]
    pub status: ToolStatus,
    /// Tekst dla modelu (po polsku; treść z zewnątrz jest obcięta i zredagowana).
    pub text: String,
    /// Dane strukturalne (zgodne z `output_schema`).
    pub data: serde_json::Value,
    /// Obrazy.
    #[serde(default)]
    pub images: Vec<ToolImage>,
    /// Treść pochodzi z zewnątrz (niezaufana) — runtime oznacza sesję i delimituje w prompcie.
    pub untrusted: Option<TaintSource>,
    /// Krok „Cofnij”.
    pub undo: Option<UndoRef>,
    /// Intencja dla UI.
    pub intent: Option<ToolIntent>,
    /// Prośba o zatwierdzenie, przez którą przeszła akcja.
    pub approval: Option<ApprovalId>,
    /// Wynik obcięty limitem.
    #[serde(default)]
    pub truncated: bool,
}

impl ToolOutcome {
    /// Sukces z tekstem i danymi.
    pub fn ok(text: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            status: ToolStatus::Ok,
            text: text.into(),
            data,
            images: Vec::new(),
            untrusted: None,
            undo: None,
            intent: None,
            approval: None,
            truncated: false,
        }
    }

    /// Odmowa z czytelnym powodem i wskazówką dla modelu.
    pub fn denied(reason: DenialReason, action: &str) -> Self {
        let text = format!(
            "Odmowa: {action} — {}. Nie ponawiaj tej samej akcji; wybierz inną drogę albo zapytaj właściciela.",
            reason.describe()
        );
        Self {
            status: ToolStatus::Denied { reason },
            ..Self::ok(text, serde_json::Value::Null)
        }
    }

    /// Błąd wykonania.
    pub fn failed(error: ToolErrorKind, text: impl Into<String>) -> Self {
        Self {
            status: ToolStatus::Failed { error },
            ..Self::ok(text, serde_json::Value::Null)
        }
    }

    /// Anulowanie.
    pub fn cancelled(action: &str) -> Self {
        Self {
            status: ToolStatus::Cancelled,
            ..Self::ok(
                format!("Anulowano: {action} (nie wykonano)."),
                serde_json::Value::Null,
            )
        }
    }

    /// Oznacza wynik jako niezaufany (builder).
    #[must_use]
    pub fn untrusted(mut self, source: TaintSource) -> Self {
        self.untrusted = Some(source);
        self
    }

    /// Czy wykonano.
    pub fn is_ok(&self) -> bool {
        self.status == ToolStatus::Ok
    }
}

/// Narzędzie agentki. Każde wywołanie przechodzi przez Brokera (`decide` → token → `verify`);
/// narzędzie nigdy nie panikuje — błędy i odmowy są wynikiem dla modelu.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Manifest.
    fn manifest(&self) -> &ToolManifest;

    /// Wywołanie z argumentami od modelu (JSON) w kontekście przebiegu.
    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome;
}

/// Parsuje argumenty zgodnie ze strukturą (zamkniętą: `deny_unknown_fields`).
pub fn parse_args<T: serde::de::DeserializeOwned>(
    args: serde_json::Value,
) -> Result<T, Box<ToolOutcome>> {
    serde_json::from_value(args).map_err(|e| {
        Box::new(ToolOutcome::failed(
            ToolErrorKind::InvalidArgs,
            format!("Niepoprawne argumenty: {e}. Popraw je zgodnie ze schematem narzędzia."),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct A {
        path: String,
    }

    #[test]
    fn outcome_builders_and_serde() {
        let d = ToolOutcome::denied(
            DenialReason::KernelBlock {
                rule: KernelRule::AuditDisable,
            },
            "czyszczenie dziennika",
        );
        assert!(d.text.contains("blokada Jądra") && d.text.contains("Nie ponawiaj"));
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["status"], "denied");
        assert_eq!(json["reason"]["reason"], "kernel_block");
        let back: ToolOutcome = serde_json::from_value(json).unwrap();
        assert_eq!(back, d);
        assert!(!d.is_ok());
        assert!(ToolOutcome::ok("x", serde_json::json!({})).is_ok());
        assert_eq!(
            ToolOutcome::cancelled("zapis").status,
            ToolStatus::Cancelled
        );
        let u = ToolOutcome::ok("t", serde_json::Value::Null).untrusted(TaintSource::File);
        assert_eq!(u.untrusted, Some(TaintSource::File));
        for r in [
            DenialReason::DenyList,
            DenialReason::OwnerDenied {
                approval: ApprovalId(1),
            },
            DenialReason::ApprovalExpired {
                approval: ApprovalId(1),
            },
            DenialReason::ApprovalTimeout {
                approval: ApprovalId(1),
            },
            DenialReason::TokenRejected,
            DenialReason::AuditUnavailable,
            DenialReason::Policy,
        ] {
            assert!(!r.describe().is_empty());
        }
    }

    #[test]
    fn args_parsing_is_strict() {
        assert!(parse_args::<A>(serde_json::json!({"path": "x"})).is_ok());
        let err = parse_args::<A>(serde_json::json!({"path": "x", "rm": true})).unwrap_err();
        assert_eq!(
            err.status,
            ToolStatus::Failed {
                error: ToolErrorKind::InvalidArgs
            }
        );
        assert!(parse_args::<A>(serde_json::json!("x")).is_err());
    }

    #[test]
    fn ctx_label_and_debug() {
        let ctx = ToolCtx::new(Holder::agent("s1", "delta")).with_workdir("C:\\w");
        let m = ToolManifest {
            name: "n".into(),
            id: "i".into(),
            title: "Zapis pliku".into(),
            description: String::new(),
            input_schema: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            reversible: risk_classifier_contract::Reversibility::Yes,
            capabilities: vec![],
            groups: vec![],
            mutating: true,
            untrusted_output: None,
        };
        assert_eq!(ctx.undo_label(&m), "delta: zapis pliku");
        let mut labelled = ctx.clone();
        labelled.label = "Delta: porządki".into();
        assert_eq!(labelled.undo_label(&m), "Delta: porządki");
        assert!(format!("{ctx:?}").contains("workdir"));
    }
}
