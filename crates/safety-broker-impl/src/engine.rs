//! Silnik Brokera: konstrukcja, Audyt każdej decyzji, wydawanie tokenów, poświadczenia IPC.

use std::sync::{Arc, Mutex, MutexGuard};

use compliance_contract::PathEnv;
use core_bus_contract::{Event, EventBus, Level};
use platform_contract::ProcessPort;
use risk_classifier_contract::{ActionFacts, AutonomyLevel, RiskClassifier, RiskVerdict};
use safety_broker_contract::ipc::{ClientCredential, ClientRole};
use safety_broker_contract::{
    BrokerError, CapToken, Capability, EVENT_KEY_ROTATED, Holder, KernelGuard, KernelPolicy,
    TokenId, event_kind, hex,
};
use watchdog_contract::{Clock, JobTable};

use crate::audit::AuditSink;
use crate::keys::{KeyMode, KeyRing, mac};
use crate::state::{State, TokenMeta};

/// Konfiguracja Brokera.
#[derive(Debug, Clone)]
pub struct BrokerConfig {
    /// Polityka Jądra (walidowana przy starcie).
    pub policy: KernelPolicy,
    /// Środowisko ścieżek (profil właściciela) — rozwijanie zmiennych w poleceniach.
    pub env: PathEnv,
    /// Źródło kluczy (`Random` w produkcji).
    pub key_mode: KeyMode,
}

/// Silnik Brokera (implementuje `Broker`, `ApprovalChannel`, `KillSwitch`, `JobRegistry`).
pub struct BrokerEngine {
    pub(crate) state: Mutex<State>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) audit: Arc<dyn AuditSink>,
    pub(crate) processes: Arc<dyn ProcessPort>,
    pub(crate) jobs: JobTable,
    pub(crate) bus: Option<Arc<dyn EventBus>>,
    pub(crate) classifier: Option<Arc<dyn RiskClassifier>>,
    pub(crate) env: PathEnv,
}

/// Maksymalne długości pól podmiotu.
const MAX_ID: usize = 128;

impl BrokerEngine {
    /// Tworzy Brokera; niepoprawna polityka albo brak CSPRNG → błąd (nie startuje).
    pub fn new(
        config: BrokerConfig,
        clock: Arc<dyn Clock>,
        audit: Arc<dyn AuditSink>,
        processes: Arc<dyn ProcessPort>,
    ) -> Result<Self, BrokerError> {
        config
            .policy
            .validate()
            .map_err(BrokerError::InvalidRequest)?;
        let keys = KeyRing::new(config.key_mode).map_err(BrokerError::InvalidRequest)?;
        let guard = KernelGuard::new(config.policy, config.env.clone());
        Ok(Self {
            state: Mutex::new(State::new(keys, guard)),
            clock,
            audit,
            processes,
            jobs: JobTable::default(),
            bus: None,
            classifier: None,
            env: config.env,
        })
    }

    /// Magistrala (zdarzenie ciszy audio przy kill-switchu).
    #[must_use]
    pub fn with_bus(mut self, bus: Arc<dyn EventBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Własny klasyfikator (domyślnie tabela z kontraktu z progami z polityki Jądra).
    #[must_use]
    pub fn with_classifier(mut self, classifier: Arc<dyn RiskClassifier>) -> Self {
        self.classifier = Some(classifier);
        self
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub(crate) fn now(&self) -> u64 {
        self.clock.now_ms()
    }

    /// Bieżąca polityka Jądra.
    pub fn policy(&self) -> KernelPolicy {
        self.lock().policy().clone()
    }

    /// Zapis Audytu (błąd → `AuditUnavailable`).
    pub(crate) fn audit(
        &self,
        name: &str,
        holder: Option<&Holder>,
        payload: serde_json::Value,
    ) -> Result<(), BrokerError> {
        let mut event = Event::new(event_kind(name), Level::Audit, payload);
        if let Some(ts) =
            chrono::DateTime::from_timestamp_millis(i64::try_from(self.now()).unwrap_or(i64::MAX))
        {
            event.ts = ts;
        }
        if let Some(h) = holder {
            event.session = Some(h.session.clone());
            event.agent = h.agent.clone();
        }
        self.audit
            .record(&event)
            .map(|_| ())
            .map_err(|e| BrokerError::AuditUnavailable(e.to_string()))
    }

    pub(crate) fn classify(
        &self,
        st: &State,
        facts: &ActionFacts,
        level: AutonomyLevel,
    ) -> RiskVerdict {
        match &self.classifier {
            Some(c) => c.evaluate(facts, level),
            None => risk_classifier_contract::evaluate(facts, level, &st.policy().risk),
        }
    }

    /// TTL po przycięciu do polityki; `Some(0)` = błąd.
    pub(crate) fn ttl(st: &State, requested: Option<u64>) -> Result<u64, BrokerError> {
        let p = st.policy();
        match requested {
            Some(0) => Err(BrokerError::InvalidRequest("TTL musi być > 0".into())),
            Some(ms) => Ok(ms.min(p.token_ttl_max_ms)),
            None => Ok(p.token_ttl_default_ms),
        }
    }

    /// Walidacja podmiotu (puste/za długie identyfikatory).
    pub(crate) fn check_holder(h: &Holder) -> Result<(), BrokerError> {
        let bad = |s: &str| s.is_empty() || s.len() > MAX_ID || s.chars().any(char::is_control);
        if bad(h.session.as_str())
            || h.agent.as_ref().is_some_and(|a| bad(a.as_str()))
            || h.role.as_deref().is_some_and(bad)
        {
            return Err(BrokerError::InvalidRequest("niepoprawny podmiot".into()));
        }
        Ok(())
    }

    /// Buduje i podpisuje token (bez rejestracji).
    pub(crate) fn mint(
        st: &mut State,
        holder: Holder,
        cap: Capability,
        parent: Option<TokenId>,
        expires_at_ms: u64,
        now: u64,
    ) -> CapToken {
        st.next_token += 1;
        let mut t = CapToken {
            id: TokenId(st.next_token),
            parent,
            cap,
            holder,
            boot: st.keys.boot(),
            key_epoch: st.keys.epoch(),
            issued_at_ms: now,
            expires_at_ms,
            mac: [0; 32],
        };
        t.mac = st.keys.sign(&t.signing_bytes());
        t
    }

    /// Rejestruje token i aktualizuje stan sesji (składnik A trifecty).
    pub(crate) fn register(st: &mut State, t: &CapToken) {
        if matches!(
            t.cap,
            Capability::FsRead(_) | Capability::SecretsRead(_) | Capability::ShellExec(_)
        ) {
            st.sessions
                .entry(t.holder.session.clone())
                .or_default()
                .private_data = true;
        }
        st.tokens.insert(
            t.id,
            TokenMeta {
                holder: t.holder.clone(),
                parent: t.parent,
                expires_at_ms: t.expires_at_ms,
            },
        );
    }

    /// Zapis zdarzenia przekazanego przez jądro/watchdoga (Broker jedynym writerem Audytu).
    pub fn append_external(
        &self,
        mut event: Event,
        relayed_by: &str,
    ) -> Result<core_log_contract::AuditRecordRef, BrokerError> {
        event.prev_hash = None;
        event.level = Level::Audit;
        match event.payload.as_object_mut() {
            Some(obj) => {
                obj.insert("relayed_by".into(), relayed_by.into());
            }
            None => {
                let value = event.payload.take();
                event.payload = serde_json::json!({ "value": value, "relayed_by": relayed_by });
            }
        }
        self.audit
            .record(&event)
            .map_err(|e| BrokerError::AuditUnavailable(e.to_string()))
    }

    /// Rotacja klucza MAC (stary klucz weryfikuje jeszcze przez maksymalny TTL tokenu).
    pub fn rotate_keys(&self) -> Result<(), BrokerError> {
        let now = self.now();
        let mut st = self.lock();
        let grace = now.saturating_add(st.policy().token_ttl_max_ms);
        st.keys.rotate(grace).map_err(BrokerError::InvalidRequest)?;
        let epoch = st.keys.epoch();
        drop(st);
        self.audit(
            EVENT_KEY_ROTATED,
            None,
            serde_json::json!({ "epoch": epoch }),
        )
    }

    fn credential_body(&self, id: &str, role: ClientRole, expires: u64) -> Vec<u8> {
        let boot = self.lock().keys.boot();
        let mut body = b"alfa-ipc-client".to_vec();
        body.extend_from_slice(&boot.0);
        body.extend_from_slice(id.as_bytes());
        body.push(0);
        body.extend_from_slice(format!("{role:?}").as_bytes());
        body.extend_from_slice(&expires.to_be_bytes());
        body
    }

    fn credential_key(&self) -> [u8; 32] {
        // Klucz pochodny od klucza bieżącego — poświadczenia giną przy kill-switchu i restarcie.
        let st = self.lock();
        st.keys.sign(b"alfa-ipc-credential-key")
    }

    /// Wydaje poświadczenie klienta IPC (przy uruchomieniu procesu klienta).
    pub fn issue_client_credential(
        &self,
        client_id: &str,
        role: ClientRole,
        ttl_ms: u64,
    ) -> ClientCredential {
        let expires_at_ms = self.now().saturating_add(ttl_ms);
        let body = self.credential_body(client_id, role, expires_at_ms);
        ClientCredential {
            client_id: client_id.to_owned(),
            role,
            expires_at_ms,
            mac: hex::encode(&mac(&self.credential_key(), &body)),
        }
    }

    /// Weryfikuje poświadczenie klienta IPC (MAC w czasie stałym, termin, uruchomienie).
    pub fn verify_client_credential(&self, c: &ClientCredential) -> Result<(), BrokerError> {
        if self.now() >= c.expires_at_ms {
            return Err(BrokerError::Unauthorized("poświadczenie wygasło".into()));
        }
        let body = self.credential_body(&c.client_id, c.role, c.expires_at_ms);
        let want = mac(&self.credential_key(), &body);
        let got = hex::decode(&c.mac).unwrap_or_default();
        if crate::keys::ct_eq(&want, &got) {
            Ok(())
        } else {
            Err(BrokerError::Unauthorized("poświadczenie odrzucone".into()))
        }
    }
}
