//! Skażenie sesji dla Brokera (przegląd fali 3, W3-04): flaga `SessionMeta.tainted` (załączniki
//! tekstowe i dokumenty z zewnątrz, `app-files`) jest trwała w katalogu sesji, a Broker decyduje
//! na swoim stanie w pamięci. Przed każdą turą (i przy otwarciu sesji) skażona sesja jest zgłaszana
//! `report_untrusted_input(…, TaintSource::File)` — także po restarcie aplikacji, więc reguła
//! `TaintedEgress` obowiązuje od pierwszego kroku agentki. Zgłoszenie jest idempotentne.

use app_api::{AppError, ErrorCode};
use safety_broker_contract::TaintSource;
use sessions_contract::{SessionCatalog, SessionId};

use crate::engine::ChatEngine;

impl ChatEngine {
    /// Zgłasza Brokerowi skażenie sesji z katalogu sesji (bez Brokera Jądra — nic: agentki nie
    /// mają wtedy narzędzi). Fail-closed: błąd odczytu sesji albo Brokera = błąd.
    pub async fn sync_taint(&self, session: &SessionId) -> Result<(), AppError> {
        let Some(broker) = &self.inner.kernel else {
            return Ok(());
        };
        let meta = self
            .inner
            .sessions
            .session(session)
            .map_err(AppError::from)?;
        if !meta.tainted {
            return Ok(());
        }
        broker
            .report_untrusted_input(session, TaintSource::File)
            .await
            .map_err(|e| {
                AppError::new(
                    ErrorCode::Unavailable,
                    format!(
                        "Broker nie przyjął oznaczenia sesji jako zawierającej treść niezaufaną \
                         (załączniki) — odpowiedź wstrzymana: {e}"
                    ),
                )
            })
    }
}
