//! Zgody „zawsze w tym zakresie” w panelu uprawnień: lista, odwołanie i zapamiętanie zgód
//! „do odwołania” (właściciel 31.10: czas zgody wybiera użytkownik — 1 h, 3 h, doba albo do
//! odwołania). Zgody z terminem żyją tylko w pamięci (restart Brokera = koniec, jak dotąd);
//! zgody do odwołania zapisuje plik w prywatnym katalogu danych Brokera (obok zapamiętanego
//! poziomu — ścieżka Jądra, agentki nie mają do niej dostępu), więc przetrwają restart, aż
//! właściciel je odwoła albo naciśnie STOP (kill-switch czyści też plik).

use std::path::PathBuf;

use core_bus_contract::{AgentId, SessionId};
use safety_broker_contract::{
    BrokerError, Capability, EVENT_GRANT_PERSIST_FAILED, EVENT_GRANT_RESTORED, EVENT_GRANT_REVOKED,
    GrantInfo, Holder, TokenId, UNTIL_REVOKED,
};
use serde::{Deserialize, Serialize};

use crate::engine::BrokerEngine;
use crate::state::{Grant, State};

/// Nazwa pliku zgód do odwołania (w katalogu pliku zapamiętanego poziomu).
pub(crate) const GRANTS_FILE: &str = "grants-standing.json";

#[derive(Serialize, Deserialize)]
struct Saved {
    session: SessionId,
    agent: Option<AgentId>,
    cap: Capability,
    granted_at_ms: u64,
}

/// Plik zgód do odwołania.
pub(crate) struct GrantStore {
    path: PathBuf,
}

impl GrantStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Zapisane zgody do odwołania; brak albo uszkodzony plik → żadnych (bezpieczny kierunek).
    fn load(&self) -> Vec<Saved> {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Zapisuje zgody do odwołania (żadnych — usuwa plik; gdy usunięcie się nie uda, np. plik
    /// trzyma antywirus, nadpisuje go pustą listą — przegląd 1.11). Zapis atomowy.
    fn save(&self, grants: &[Grant]) -> std::io::Result<()> {
        let keep: Vec<Saved> = grants
            .iter()
            .filter(|g| g.until_ms == UNTIL_REVOKED)
            .map(|g| Saved {
                session: g.session.clone(),
                agent: g.agent.clone(),
                cap: g.cap.clone(),
                granted_at_ms: g.granted_at_ms,
            })
            .collect();
        if keep.is_empty() {
            return match std::fs::remove_file(&self.path) {
                // Zapis w miejscu (bez zmiany nazwy): uchwyt bez prawa usuwania go nie blokuje.
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    std::fs::write(&self.path, "[]")
                }
                _ => Ok(()),
            };
        }
        let text = serde_json::to_string(&keep).map_err(std::io::Error::other)?;
        self.write(&text)
    }

    fn write(&self, text: &str) -> std::io::Result<()> {
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)
    }
}

impl BrokerEngine {
    /// Wczytuje zgody do odwołania zapisane przed restartem.
    pub(crate) fn restore_grants(&self, store: &GrantStore) {
        let now = self.now();
        let mut st = self.lock();
        let saved = store.load();
        if !saved.is_empty() {
            let payload = serde_json::json!({ "grants": saved.len() });
            let _ = self.audit(EVENT_GRANT_RESTORED, None, payload);
        }
        for s in saved {
            let id = next_id(&mut st, now);
            st.grants.push(Grant {
                id,
                session: s.session,
                agent: s.agent,
                cap: s.cap,
                granted_at_ms: s.granted_at_ms,
                until_ms: UNTIL_REVOKED,
            });
        }
    }

    /// Utrwala zgody do odwołania (błąd zapisu → wpis w Audycie pod własną nazwą; zgody w pamięci
    /// obowiązują).
    pub(crate) fn persist_grants(&self, st: &State) {
        if let Some(store) = &self.grant_store
            && let Err(e) = store.save(&st.grants)
        {
            let payload = serde_json::json!({ "persist_error": e.to_string() });
            let _ = self.audit(EVENT_GRANT_PERSIST_FAILED, None, payload);
        }
    }

    /// Obowiązujące zgody (wygasłe usunięte).
    pub(crate) fn grants_sync(&self) -> Vec<GrantInfo> {
        let now = self.now();
        let mut st = self.lock();
        st.grants.retain(|g| now < g.until_ms);
        st.grants
            .iter()
            .map(|g| GrantInfo {
                id: g.id,
                session: g.session.clone(),
                agent: g.agent.clone(),
                scope: g.cap.clone(),
                granted_at_ms: g.granted_at_ms,
                until_ms: g.until_ms,
            })
            .collect()
    }

    /// Odwołuje zgodę: znika z pamięci i z pliku, tokeny wydane na jej podstawie tracą ważność,
    /// wpis w Audycie. Odwołanie obowiązuje także bez zapisu w Audycie (zawęża uprawnienia —
    /// jak unieważnienie tokenu; przegląd 1.11: wcześniej błąd Audytu przerywał przed plikiem).
    pub(crate) fn revoke_grant_sync(&self, id: u64) -> Result<bool, BrokerError> {
        let mut st = self.lock();
        let Some(pos) = st.grants.iter().position(|g| g.id == id) else {
            return Ok(false);
        };
        let g = st.grants.remove(pos);
        let holder = Holder {
            session: g.session.clone(),
            agent: g.agent.clone(),
            role: None,
        };
        self.persist_grants(&st);
        let tokens: Vec<TokenId> = st
            .tokens
            .iter()
            .filter(|(_, meta)| meta.grant == Some(id))
            .map(|(token, _)| *token)
            .collect();
        let revoked = self.revoke_ids(&mut st, &tokens, "grant_revoked");
        let payload = serde_json::json!({
            "grant": id, "scope": g.cap, "until_ms": g.until_ms, "tokens_revoked": revoked,
        });
        let _ = self.audit(EVENT_GRANT_REVOKED, Some(&holder), payload);
        Ok(true)
    }
}

/// Kolejny numer zgody: rośnie także między uruchomieniami Brokera (co najmniej bieżąca chwila
/// w ms), więc „Odwołaj” z nieodświeżonej listy po restarcie nie trafi w inną zgodę (przegląd 1.11).
pub(crate) fn next_id(st: &mut State, now: u64) -> u64 {
    st.next_grant = st.next_grant.saturating_add(1).max(now);
    st.next_grant
}
