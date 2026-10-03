//! Sesje przeglądarki per sesja rozmowy i ich filtr egressu: hosty zatwierdzone przez Brokera
//! (`net.egress(host)`), zawsze z deny-listą domen dostawców (nawet gdyby zgoda istniała).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};

use compliance_contract::DenyChecker;
use core_bus_contract::SessionId;
use platform_apps_contract::{BrowserSessionId, EgressFilter};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Hosty zatwierdzone w sesji przeglądarki.
pub(crate) struct HostAllow {
    hosts: Mutex<BTreeSet<String>>,
    deny: Arc<DenyChecker>,
}

impl HostAllow {
    pub(crate) fn new(deny: Arc<DenyChecker>) -> Self {
        Self {
            hosts: Mutex::new(BTreeSet::new()),
            deny,
        }
    }

    pub(crate) fn grant(&self, host: &str) {
        lock(&self.hosts).insert(host.to_ascii_lowercase());
    }

    pub(crate) fn contains(&self, host: &str) -> bool {
        lock(&self.hosts).contains(&host.to_ascii_lowercase())
    }

    pub(crate) fn hosts(&self) -> Vec<String> {
        lock(&self.hosts).iter().cloned().collect()
    }

    pub(crate) fn len(&self) -> usize {
        lock(&self.hosts).len()
    }
}

impl EgressFilter for HostAllow {
    fn allows(&self, host: &str) -> bool {
        !self.deny.is_denied_domain(host) && self.contains(host)
    }
}

/// Sesja przeglądarki jednej sesji rozmowy.
#[derive(Clone)]
pub(crate) struct Browsing {
    pub(crate) id: BrowserSessionId,
    pub(crate) allow: Arc<HostAllow>,
    /// Adres bieżącej strony (z ostatniego wyniku portu).
    url: Arc<Mutex<String>>,
}

impl Browsing {
    pub(crate) fn new(id: BrowserSessionId, allow: Arc<HostAllow>) -> Self {
        Self {
            id,
            allow,
            url: Arc::new(Mutex::new(String::new())),
        }
    }

    pub(crate) fn set_url(&self, url: &str) {
        *lock(&self.url) = url.to_owned();
    }

    pub(crate) fn url(&self) -> String {
        lock(&self.url).clone()
    }
}

/// Rejestr sesji przeglądarki.
#[derive(Default)]
pub(crate) struct Sessions {
    map: Mutex<BTreeMap<SessionId, Browsing>>,
}

impl Sessions {
    pub(crate) fn get(&self, s: &SessionId) -> Option<Browsing> {
        lock(&self.map).get(s).cloned()
    }

    pub(crate) fn insert(&self, s: SessionId, b: Browsing) {
        lock(&self.map).insert(s, b);
    }

    pub(crate) fn remove(&self, s: &SessionId) -> Option<Browsing> {
        lock(&self.map).remove(s)
    }

    pub(crate) fn drain(&self) -> Vec<Browsing> {
        std::mem::take(&mut *lock(&self.map))
            .into_values()
            .collect()
    }
}
