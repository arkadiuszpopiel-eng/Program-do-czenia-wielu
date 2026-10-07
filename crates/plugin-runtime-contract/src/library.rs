//! Rdzeń biblioteki (wspólny dla `-impl` i `-fake`): propozycja → zatwierdzenie właściciela
//! (kanał UI + hash przejrzanego manifestu) → instalacja; aktualizacja = wyższa wersja (stara
//! „zastąpiona”); wyłączenie; ponowne włączenie po ponownym zatwierdzeniu; usunięcie.
//! Każda zmiana zwraca zdarzenia `plugin.*` (bez treści: id, wersja, hashe, stan).

use core_bus_contract::{Event, Level};
use semver::Version;
use tools_common_contract::ToolManifest;

use crate::error::PluginError;
use crate::manifest::{PluginId, PluginManifest};
use crate::model::{ApprovalOrigin, PluginApproval, PluginRecord, PluginSource, PluginState};
use crate::validate::{check_approved, review_hash, validate_manifest};
use crate::{event_kind, events};

/// Zdarzenie cyklu życia dla rekordu.
pub fn record_event(name: &str, level: Level, r: &PluginRecord) -> Event {
    Event::new(
        event_kind(name),
        level,
        serde_json::json!({
            "plugin": r.manifest.id,
            "version": r.manifest.version.to_string(),
            "review_hash": r.review_hash,
            "wasm_sha256": r.manifest.wasm_sha256,
            "state": r.state,
            "capabilities": r.manifest.families(),
        }),
    )
}

/// Biblioteka wtyczek.
#[derive(Debug, Clone, Default)]
pub struct PluginLibrary {
    records: Vec<PluginRecord>,
}

type Changed<T> = Result<(T, Vec<Event>), PluginError>;

impl PluginLibrary {
    /// Biblioteka nad zapisanymi rekordami.
    pub fn new(records: Vec<PluginRecord>) -> Self {
        Self { records }
    }

    /// Wszystkie wersje.
    pub fn records(&self) -> &[PluginRecord] {
        &self.records
    }

    /// Aktywna (zainstalowana) wersja.
    pub fn installed(&self, id: &PluginId) -> Option<&PluginRecord> {
        self.records
            .iter()
            .find(|r| &r.manifest.id == id && r.state == PluginState::Installed)
    }

    /// Zainstalowane wersje z nienaruszonym zatwierdzeniem (narzędzia w rejestrze agentek).
    /// Rekord zmieniony poza biblioteką (manifest ≠ zatwierdzony hash) nie jest aktywny.
    pub fn active(&self) -> impl Iterator<Item = &PluginRecord> {
        self.records
            .iter()
            .filter(|r| r.is_active() && check_approved(r).is_ok())
    }

    /// Manifesty narzędzi aktywnych wtyczek.
    pub fn tool_catalog(&self) -> Vec<ToolManifest> {
        self.active()
            .flat_map(|r| r.manifest.tool_manifests())
            .collect()
    }

    fn position(&self, id: &PluginId, version: &Version) -> Result<usize, PluginError> {
        self.records
            .iter()
            .position(|r| &r.manifest.id == id && &r.manifest.version == version)
            .ok_or_else(|| PluginError::NotFound(format!("{id}@{version}")))
    }

    /// Nazwa narzędzia, którą dostarcza już inna aktywna wtyczka.
    fn conflict(&self, m: &PluginManifest) -> Option<String> {
        let mine: Vec<String> = m.tools.iter().map(PluginManifest::tool_name).collect();
        self.active()
            .filter(|r| r.manifest.id != m.id)
            .flat_map(|r| r.manifest.tools.iter().map(PluginManifest::tool_name))
            .find(|n| mine.contains(n))
    }

    /// Propozycja wersji (manifest zwalidowany; bajty sprawdza wywołujący: hash i komponent).
    pub fn propose(
        &mut self,
        manifest: PluginManifest,
        source: PluginSource,
        now_ms: u64,
    ) -> Changed<PluginRecord> {
        validate_manifest(&manifest)?;
        let hash = review_hash(&manifest)?;
        if let Ok(i) = self.position(&manifest.id, &manifest.version) {
            let existing = &self.records[i];
            return if existing.review_hash == hash {
                Ok((existing.clone(), Vec::new()))
            } else {
                Err(PluginError::VersionExists(manifest.version.to_string()))
            };
        }
        if let Some(inst) = self.installed(&manifest.id)
            && manifest.version <= inst.manifest.version
        {
            return Err(PluginError::NotNewer(manifest.version.to_string()));
        }
        if let Some(name) = self.conflict(&manifest) {
            return Err(PluginError::ToolConflict(name));
        }
        let record = PluginRecord {
            manifest,
            review_hash: hash,
            source,
            state: PluginState::Proposed,
            approval: None,
            proposed_at_ms: now_ms,
            decided_at_ms: None,
        };
        let ev = vec![record_event(events::PROPOSED, Level::Info, &record)];
        self.records.push(record.clone());
        Ok((record, ev))
    }

    fn check_approval(r: &PluginRecord, approval: &PluginApproval) -> Result<(), PluginError> {
        if approval.origin != ApprovalOrigin::Ui {
            return Err(PluginError::ApprovalChannel);
        }
        if approval.reviewed_hash != r.review_hash {
            return Err(PluginError::HashMismatch);
        }
        Ok(())
    }

    /// Zatwierdzenie propozycji → instalacja (starsza zainstalowana → „zastąpiona”).
    pub fn approve(
        &mut self,
        id: &PluginId,
        version: &Version,
        approval: PluginApproval,
        now_ms: u64,
    ) -> Changed<PluginRecord> {
        let i = self.position(id, version)?;
        if self.records[i].state != PluginState::Proposed {
            return Err(PluginError::WrongState(self.records[i].state));
        }
        Self::check_approval(&self.records[i], &approval)?;
        if let Some(name) = self.conflict(&self.records[i].manifest) {
            return Err(PluginError::ToolConflict(name));
        }
        let mut ev = Vec::new();
        if let Some(j) = self
            .records
            .iter()
            .position(|r| &r.manifest.id == id && r.state == PluginState::Installed)
        {
            if self.records[j].manifest.version >= *version {
                return Err(PluginError::NotNewer(version.to_string()));
            }
            self.records[j].state = PluginState::Superseded;
            self.records[j].decided_at_ms = Some(now_ms);
            ev.push(record_event(
                events::SUPERSEDED,
                Level::Info,
                &self.records[j],
            ));
        }
        for r in self
            .records
            .iter_mut()
            .filter(|r| &r.manifest.id == id && r.state == PluginState::Disabled)
        {
            r.state = PluginState::Superseded;
            r.decided_at_ms = Some(now_ms);
        }
        let r = &mut self.records[i];
        r.state = PluginState::Installed;
        r.approval = Some(approval);
        r.decided_at_ms = Some(now_ms);
        ev.push(record_event(events::INSTALLED, Level::Info, r));
        Ok((r.clone(), ev))
    }

    /// Odrzucenie propozycji.
    pub fn reject(
        &mut self,
        id: &PluginId,
        version: &Version,
        now_ms: u64,
    ) -> Changed<PluginRecord> {
        let i = self.position(id, version)?;
        let r = &mut self.records[i];
        if r.state != PluginState::Proposed {
            return Err(PluginError::WrongState(r.state));
        }
        r.state = PluginState::Rejected;
        r.decided_at_ms = Some(now_ms);
        Ok((
            r.clone(),
            vec![record_event(events::REJECTED, Level::Info, r)],
        ))
    }

    /// Wyłączenie zainstalowanej wersji (narzędzia znikają z rejestru).
    pub fn disable(&mut self, id: &PluginId, now_ms: u64) -> Changed<PluginRecord> {
        let i = self
            .records
            .iter()
            .position(|r| &r.manifest.id == id && r.state == PluginState::Installed)
            .ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        let r = &mut self.records[i];
        r.state = PluginState::Disabled;
        r.decided_at_ms = Some(now_ms);
        Ok((
            r.clone(),
            vec![record_event(events::DISABLED, Level::Info, r)],
        ))
    }

    /// Ponowne włączenie wyłączonej wersji — tylko z ponownym zatwierdzeniem (UI + hash).
    pub fn enable(
        &mut self,
        id: &PluginId,
        approval: PluginApproval,
        now_ms: u64,
    ) -> Changed<PluginRecord> {
        let i = self
            .records
            .iter()
            .position(|r| &r.manifest.id == id && r.state == PluginState::Disabled)
            .ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        Self::check_approval(&self.records[i], &approval)?;
        if let Some(name) = self.conflict(&self.records[i].manifest) {
            return Err(PluginError::ToolConflict(name));
        }
        let r = &mut self.records[i];
        r.state = PluginState::Installed;
        r.approval = Some(approval);
        r.decided_at_ms = Some(now_ms);
        Ok((
            r.clone(),
            vec![record_event(events::ENABLED, Level::Info, r)],
        ))
    }

    /// Usunięcie wszystkich wersji wtyczki (bajty modułów usuwa wywołujący).
    pub fn remove(&mut self, id: &PluginId) -> Changed<Vec<PluginRecord>> {
        let (gone, kept): (Vec<_>, Vec<_>) =
            self.records.drain(..).partition(|r| &r.manifest.id == id);
        self.records = kept;
        if gone.is_empty() {
            return Err(PluginError::NotFound(id.to_string()));
        }
        let ev = gone
            .iter()
            .map(|r| record_event(events::REMOVED, Level::Info, r))
            .collect();
        Ok((gone, ev))
    }

    /// Czy jakaś wersja (poza usuniętymi) używa modułu o tym hashu.
    pub fn uses_wasm(&self, sha256: &str) -> bool {
        self.records
            .iter()
            .any(|r| r.manifest.wasm_sha256 == sha256)
    }
}
