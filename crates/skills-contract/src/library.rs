//! Rdzeń biblioteki (wspólny dla `-impl` i `-fake`): cykl życia wersji (propozycja →
//! [kwarantanna →] zatwierdzenie właściciela → instalacja; aktualizacja = nowsza wersja, stara
//! „zastąpiona”), eksport/import paczek. Każda zmiana zwraca zdarzenia `skills.*` (bez treści).

use core_bus_contract::{Event, Level};
use semver::Version;

use crate::bundle::{BundledSkill, SkillBundle, content_hash};
use crate::error::SkillError;
use crate::model::{
    ApprovalOrigin, ImportOrigin, OwnerApproval, Skill, SkillId, SkillRecord, SkillSource,
    SkillState,
};
use crate::validate::{run_acceptance, scan, validate_skill};
use crate::{event_kind, events};
use tools_common_contract::ToolManifest;

/// Raport importu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Przyjęte jako propozycje (identyfikator, wersja, stan).
    pub proposed: Vec<(SkillId, String, SkillState)>,
    /// Pominięte (identyfikator, powód).
    pub skipped: Vec<(String, String)>,
}

/// Biblioteka umiejętności.
#[derive(Debug, Clone, Default)]
pub struct SkillLibrary {
    records: Vec<SkillRecord>,
    catalog: Vec<ToolManifest>,
}

fn event(name: &str, level: Level, r: &SkillRecord) -> Event {
    Event::new(
        event_kind(name),
        level,
        serde_json::json!({
            "skill": r.skill.id,
            "version": r.skill.version.to_string(),
            "hash": r.hash,
            "state": r.state,
            "findings": r.findings.len(),
        }),
    )
}

impl SkillLibrary {
    /// Biblioteka nad katalogiem narzędzi i zapisanymi wersjami.
    pub fn new(catalog: Vec<ToolManifest>, records: Vec<SkillRecord>) -> Self {
        Self { records, catalog }
    }

    /// Wszystkie wersje.
    pub fn records(&self) -> &[SkillRecord] {
        &self.records
    }

    /// Katalog narzędzi.
    pub fn catalog(&self) -> &[ToolManifest] {
        &self.catalog
    }

    /// Zainstalowana wersja.
    pub fn installed(&self, id: &SkillId) -> Option<&SkillRecord> {
        self.records
            .iter()
            .find(|r| &r.skill.id == id && r.state == SkillState::Installed)
    }

    fn position(&self, id: &SkillId, version: &Version) -> Result<usize, SkillError> {
        self.records
            .iter()
            .position(|r| &r.skill.id == id && &r.skill.version == version)
            .ok_or_else(|| SkillError::NotFound(id.clone()))
    }

    /// Propozycja nowej umiejętności albo wersji: walidacja, testy akceptacyjne, skaner treści.
    /// Źródło niezaufane albo podejrzana treść spoza właściciela → kwarantanna.
    pub fn propose(
        &mut self,
        skill: Skill,
        source: SkillSource,
        now_ms: u64,
    ) -> Result<(SkillRecord, Vec<Event>), SkillError> {
        validate_skill(&skill, &self.catalog)?;
        run_acceptance(&skill)?;
        let hash = content_hash(&skill).map_err(SkillError::Invalid)?;
        if let Ok(i) = self.position(&skill.id, &skill.version) {
            let existing = &self.records[i];
            return if existing.hash == hash {
                Ok((existing.clone(), Vec::new()))
            } else {
                Err(SkillError::VersionExists(skill.version.to_string()))
            };
        }
        if let Some(inst) = self.installed(&skill.id)
            && skill.version <= inst.skill.version
        {
            return Err(SkillError::NotNewer(skill.version.to_string()));
        }
        let findings = scan(&skill);
        let quarantine =
            source.is_untrusted() || (!findings.is_empty() && source != SkillSource::User);
        let record = SkillRecord {
            skill,
            hash,
            source,
            state: if quarantine {
                SkillState::Quarantined
            } else {
                SkillState::Proposed
            },
            findings,
            approval: None,
            proposed_at_ms: now_ms,
            decided_at_ms: None,
        };
        let name = if quarantine {
            events::QUARANTINED
        } else {
            events::PROPOSED
        };
        let ev = vec![event(name, Level::Info, &record)];
        self.records.push(record.clone());
        Ok((record, ev))
    }

    fn install(
        &mut self,
        i: usize,
        approval: OwnerApproval,
        now_ms: u64,
    ) -> Result<(SkillRecord, Vec<Event>), SkillError> {
        if approval.reviewed_hash != self.records[i].hash {
            return Err(SkillError::HashMismatch);
        }
        let id = self.records[i].skill.id.clone();
        let version = self.records[i].skill.version.clone();
        let mut ev = Vec::new();
        if let Some(j) = self
            .records
            .iter()
            .position(|r| r.skill.id == id && r.state == SkillState::Installed)
        {
            if self.records[j].skill.version >= version {
                return Err(SkillError::NotNewer(version.to_string()));
            }
            self.records[j].state = SkillState::Superseded;
            self.records[j].decided_at_ms = Some(now_ms);
            ev.push(event(events::SUPERSEDED, Level::Info, &self.records[j]));
        }
        let r = &mut self.records[i];
        r.state = SkillState::Installed;
        r.approval = Some(approval);
        r.decided_at_ms = Some(now_ms);
        ev.push(event(events::INSTALLED, Level::Info, r));
        Ok((r.clone(), ev))
    }

    /// Zatwierdzenie propozycji przez właściciela (hash przejrzanej treści) → instalacja.
    pub fn approve(
        &mut self,
        id: &SkillId,
        version: &Version,
        approval: OwnerApproval,
        now_ms: u64,
    ) -> Result<(SkillRecord, Vec<Event>), SkillError> {
        let i = self.position(id, version)?;
        match self.records[i].state {
            SkillState::Proposed => self.install(i, approval, now_ms),
            other => Err(SkillError::WrongState(other)),
        }
    }

    /// Zwolnienie z kwarantanny — tylko zatwierdzenie w oknie (nie głos/tekst) → instalacja.
    pub fn release(
        &mut self,
        id: &SkillId,
        version: &Version,
        approval: OwnerApproval,
        now_ms: u64,
    ) -> Result<(SkillRecord, Vec<Event>), SkillError> {
        let i = self.position(id, version)?;
        if self.records[i].state != SkillState::Quarantined {
            return Err(SkillError::WrongState(self.records[i].state));
        }
        if approval.origin != ApprovalOrigin::Ui {
            return Err(SkillError::ApprovalChannel);
        }
        let (r, mut ev) = self.install(i, approval, now_ms)?;
        ev.insert(0, event(events::RELEASED, Level::Warn, &r));
        Ok((r, ev))
    }

    /// Odrzucenie propozycji albo umiejętności z kwarantanny.
    pub fn reject(
        &mut self,
        id: &SkillId,
        version: &Version,
        now_ms: u64,
    ) -> Result<(SkillRecord, Vec<Event>), SkillError> {
        let i = self.position(id, version)?;
        let r = &mut self.records[i];
        if !matches!(r.state, SkillState::Proposed | SkillState::Quarantined) {
            return Err(SkillError::WrongState(r.state));
        }
        r.state = SkillState::Rejected;
        r.decided_at_ms = Some(now_ms);
        Ok((r.clone(), vec![event(events::REJECTED, Level::Info, r)]))
    }

    /// Wyłączenie zainstalowanej umiejętności.
    pub fn disable(
        &mut self,
        id: &SkillId,
        now_ms: u64,
    ) -> Result<(SkillRecord, Vec<Event>), SkillError> {
        let i = self
            .records
            .iter()
            .position(|r| &r.skill.id == id && r.state == SkillState::Installed)
            .ok_or_else(|| SkillError::NotFound(id.clone()))?;
        let r = &mut self.records[i];
        r.state = SkillState::Disabled;
        r.decided_at_ms = Some(now_ms);
        Ok((r.clone(), vec![event(events::DISABLED, Level::Info, r)]))
    }

    /// Eksport zainstalowanych (wszystkich albo wskazanych).
    pub fn export(&self, ids: &[SkillId]) -> Result<SkillBundle, SkillError> {
        let skills = self
            .records
            .iter()
            .filter(|r| r.state == SkillState::Installed)
            .filter(|r| ids.is_empty() || ids.contains(&r.skill.id))
            .map(|r| BundledSkill {
                skill: r.skill.clone(),
                source: r.source.clone(),
            })
            .collect();
        SkillBundle::new(skills).map_err(SkillError::Bundle)
    }

    /// Import paczki: weryfikacja hasha, każda umiejętność jako propozycja (z zewnątrz —
    /// kwarantanna); błędy pojedynczych pozycji nie przerywają importu.
    pub fn import(
        &mut self,
        bundle: &SkillBundle,
        origin: ImportOrigin,
        now_ms: u64,
    ) -> Result<(ImportReport, Vec<Event>), SkillError> {
        bundle.verify().map_err(SkillError::Bundle)?;
        let mut report = ImportReport::default();
        let mut ev = Vec::new();
        for b in &bundle.skills {
            match self.propose(b.skill.clone(), SkillSource::Import { origin }, now_ms) {
                Ok((r, e)) => {
                    report.proposed.push((
                        r.skill.id.clone(),
                        r.skill.version.to_string(),
                        r.state,
                    ));
                    ev.extend(e);
                }
                Err(e) => report.skipped.push((b.skill.id.to_string(), e.to_string())),
            }
        }
        ev.push(Event::new(
            event_kind(events::IMPORTED),
            Level::Info,
            serde_json::json!({ "proposed": report.proposed.len(), "skipped": report.skipped.len(), "origin": origin }),
        ));
        Ok((report, ev))
    }
}
