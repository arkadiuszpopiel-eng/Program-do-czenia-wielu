//! Atrapa modułu zgodności (docs/modules/compliance/SPEC.md „Fake”): rejestr w pamięci,
//! sterowana data „dziś”, sterowane statusy tras, rejestr wywołań `route_allowed`.
//! Decyzje liczy ta sama czysta logika z kontraktu (`RouteTable`), co w `-impl`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use chrono::NaiveDate;
use compliance_contract::{
    ChangeOrigin, Compliance, ComplianceError, Decision, DenyChecker, DenyLists, KernelAuthority,
    PathEnv, ProviderPolicyInput, Registry, RegistryRoute, RouteId, RouteStatus, RouteTable,
    RouteView, SessionTag, TableSettings,
};

/// Profil użytkownika używany do rozwijania `%USERPROFILE%` w atrapie.
pub const FAKE_PROFILE: &str = r"C:\Users\Test";

struct State {
    table: RouteTable,
    deny: DenyChecker,
    today: NaiveDate,
    queries: Vec<(RouteId, SessionTag, bool)>,
}

/// Atrapa `Compliance`.
pub struct FakeCompliance {
    env: PathEnv,
    state: Mutex<State>,
}

impl FakeCompliance {
    /// Atrapa z rejestru, wpisów katalogu i daty „dziś”.
    pub fn new(registry: Registry, catalog: Vec<ProviderPolicyInput>, today: NaiveDate) -> Self {
        let env = PathEnv::windows_profile(FAKE_PROFILE);
        Self {
            state: Mutex::new(State {
                table: RouteTable::new(registry, catalog, TableSettings::default()),
                deny: DenyChecker::new(DenyLists::baseline(), &env),
                today,
                queries: Vec::new(),
            }),
            env,
        }
    }

    /// Ustawia datę „dziś”.
    pub fn set_today(&self, today: NaiveDate) {
        self.lock().today = today;
    }

    /// Przesuwa „dziś” o podaną liczbę dni.
    pub fn advance_days(&self, days: i64) {
        let mut st = self.lock();
        st.today += chrono::Duration::days(days);
    }

    /// Zmienia zadeklarowany status trasy z rejestru; `false`, gdy trasy nie ma.
    pub fn set_status(&self, id: &RouteId, status: RouteStatus) -> bool {
        self.lock().table.set_declared_status(id, status)
    }

    /// Wszystkie zapytania `route_allowed`: (trasa, tag sesji, decyzja).
    pub fn queries(&self) -> Vec<(RouteId, SessionTag, bool)> {
        self.lock().queries.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[async_trait]
impl Compliance for FakeCompliance {
    fn route(&self, id: &RouteId) -> Option<RegistryRoute> {
        self.lock().table.registry().route(id).cloned()
    }

    fn view(&self, id: &RouteId) -> Option<RouteView> {
        let st = self.lock();
        st.table.view(id, st.today)
    }

    fn views(&self) -> Vec<RouteView> {
        let st = self.lock();
        st.table.views(st.today)
    }

    fn route_allowed(&self, id: &RouteId, session: SessionTag) -> Decision {
        let mut st = self.lock();
        let decision = st.table.decide(id, session, st.today);
        st.queries.push((id.clone(), session, decision.allowed));
        decision
    }

    async fn set_enabled(
        &self,
        id: &RouteId,
        on: bool,
        origin: ChangeOrigin,
    ) -> Result<RouteView, ComplianceError> {
        let mut st = self.lock();
        let today = st.today;
        st.table
            .toggle(id, on, &origin, today)
            .map(|(_, after)| after)
    }

    fn deny_lists(&self) -> DenyLists {
        self.lock().deny.lists().clone()
    }

    fn is_denied_path(&self, path: &str) -> bool {
        self.lock().deny.is_denied_path(path, &self.env)
    }

    fn is_denied_domain(&self, domain: &str) -> bool {
        self.lock().deny.is_denied_domain(domain)
    }

    async fn replace_deny_lists(
        &self,
        _authority: &KernelAuthority,
        lists: DenyLists,
    ) -> Result<(), ComplianceError> {
        lists.validate()?;
        self.lock().deny = DenyChecker::new(lists, &self.env);
        Ok(())
    }
}
