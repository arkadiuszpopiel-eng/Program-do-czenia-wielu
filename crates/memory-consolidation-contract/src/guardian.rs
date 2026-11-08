//! Przebieg Strażniczki pamięci: polityka startu → per zakres (sesje, potem projekt/agentka,
//! globalna) reguły deterministyczne (retencja, duplikaty, sprzeczności) → model językowy
//! (budżet tła, tylko epizody zaufane, sesje prywatne tylko lokalnie) → zmiany z dziennikiem →
//! propozycje awansu (oczekujące). Między zakresami ponowne sprawdzenie stanu maszyny.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use memory_contract::{
    Accessor, ChangeOp, ChangeSet, EntryState, EventSink, InspectorQuery, Layer, MAX_PAGE,
    MemoryClock, MemoryEntry, MemoryError, MemoryScope, MemoryService, PrivacyOracle, entry_state,
    events as names, scope_key,
};
use serde_json::json;

use crate::config::{AutoExtract, ConsolidationConfig, Trigger};
use crate::policy::{SkipReason, may_start};
use crate::ports::{
    BackgroundBudget, BudgetVerdict, ConsolidationBatch, Consolidator, EpisodeView, FactView,
    HostConditions,
};
use crate::report::{RunReport, ScopeRun};
use crate::rules;

/// Porty Strażniczki.
#[derive(Clone)]
pub struct GuardianPorts {
    /// Pamięć.
    pub memory: Arc<dyn MemoryService>,
    /// Model (brak = tylko reguły deterministyczne).
    pub consolidator: Option<Arc<dyn Consolidator>>,
    /// Budżet tła.
    pub budget: Arc<dyn BackgroundBudget>,
    /// Stan maszyny.
    pub host: Arc<dyn HostConditions>,
    /// Prywatność sesji.
    pub privacy: Arc<dyn PrivacyOracle>,
    /// Zdarzenia.
    pub events: Arc<dyn EventSink>,
    /// Zegar.
    pub clock: Arc<dyn MemoryClock>,
}

/// Strażniczka pamięci (zadanie tła).
pub struct Guardian {
    pub(crate) ports: GuardianPorts,
    pub(crate) config: ConsolidationConfig,
    running: AtomicBool,
}

fn err(e: &MemoryError) -> String {
    match e {
        MemoryError::Storage { .. } => "błąd magazynu".into(),
        other => other.to_string(),
    }
}

impl Guardian {
    /// Nowa Strażniczka.
    pub fn new(ports: GuardianPorts, config: ConsolidationConfig) -> Self {
        Self {
            ports,
            config,
            running: AtomicBool::new(false),
        }
    }

    /// Konfiguracja.
    pub fn config(&self) -> &ConsolidationConfig {
        &self.config
    }

    pub(crate) fn entries(&self, scope: &MemoryScope) -> Result<Vec<MemoryEntry>, MemoryError> {
        let mut out = Vec::new();
        loop {
            let q = InspectorQuery {
                scopes: vec![scope.clone()],
                offset: out.len(),
                limit: MAX_PAGE,
                ..InspectorQuery::default()
            };
            let page = self.ports.memory.inspect(&Accessor::Guardian, &q)?;
            let n = page.items.len();
            out.extend(page.items.into_iter().map(|i| i.entry));
            if n == 0 || out.len() >= page.total {
                break;
            }
        }
        out.sort_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
        Ok(out)
    }

    /// Jeden przebieg. Nigdy nie zwraca błędu — błędy trafiają do raportu.
    pub async fn run(&self, trigger: Trigger) -> RunReport {
        let now = self.ports.clock.now();
        let mut report = RunReport {
            run: format!("noc-{}", now.format("%Y%m%dT%H%M%S")),
            trigger,
            started_at: now,
            skipped: None,
            interrupted: None,
            scopes: Vec::new(),
            llm_calls: 0,
            budget_denied: false,
            proposals: 0,
            errors: Vec::new(),
        };
        if let Err(reason) = may_start(&self.config, &self.ports.host.state(), trigger) {
            return self.skip(report, reason);
        }
        if self.running.swap(true, Ordering::SeqCst) {
            return self.skip(report, SkipReason::AlreadyRunning);
        }
        self.emit(
            names::CONSOLIDATION_STARTED,
            json!({ "run": report.run, "trigger": trigger }),
        );
        self.run_scopes(&mut report, trigger).await;
        if report.interrupted.is_none() && self.config.propose_promotions {
            self.propose_promotions(&mut report);
        }
        self.running.store(false, Ordering::SeqCst);
        let totals: (usize, usize, usize) = report.scopes.iter().fold((0, 0, 0), |t, s| {
            (
                t.0 + s.created,
                t.1 + s.merged + s.resolved,
                t.2 + s.expired,
            )
        });
        self.emit(
            names::CONSOLIDATION_FINISHED,
            json!({ "run": report.run, "scopes": report.scopes.len(), "created": totals.0,
                    "merged_or_resolved": totals.1, "expired": totals.2, "llm_calls": report.llm_calls,
                    "proposals": report.proposals, "interrupted": report.interrupted,
                    "budget_denied": report.budget_denied, "errors": report.errors.len() }),
        );
        report
    }

    fn skip(&self, mut report: RunReport, reason: SkipReason) -> RunReport {
        report.skipped = Some(reason);
        self.emit(
            names::CONSOLIDATION_SKIPPED,
            json!({ "run": report.run, "reason": reason }),
        );
        report
    }

    fn emit(&self, kind: &str, payload: serde_json::Value) {
        self.ports.events.emit(kind, None, payload);
    }

    async fn run_scopes(&self, report: &mut RunReport, trigger: Trigger) {
        let mut scopes: Vec<MemoryScope> = match self.ports.memory.scopes(&Accessor::Guardian) {
            Ok(s) => s.into_iter().map(|s| s.scope).collect(),
            Err(e) => {
                report.errors.push(err(&e));
                return;
            }
        };
        scopes.sort_by_key(|s| match s {
            MemoryScope::Session(_) => 0,
            MemoryScope::Project(_) | MemoryScope::Agent(_) => 1,
            MemoryScope::Global => 2,
        });
        for scope in scopes {
            if let Err(reason) = may_start(&self.config, &self.ports.host.state(), trigger) {
                report.interrupted = Some(reason);
                break;
            }
            match self.run_scope(&scope, report).await {
                Ok(run) => report.scopes.push(run),
                Err(e) => report
                    .errors
                    .push(format!("{}: {}", scope_key(&scope), err(&e))),
            }
        }
    }

    async fn run_scope(
        &self,
        scope: &MemoryScope,
        report: &mut RunReport,
    ) -> Result<ScopeRun, MemoryError> {
        let now = self.ports.clock.now();
        let memory = &self.ports.memory;
        let mut run = ScopeRun {
            scope: scope_key(scope),
            ..ScopeRun::default()
        };
        let entries = self.entries(scope)?;
        let journal = memory.journal(&Accessor::Guardian, scope)?;
        let mut ops = rules::retention(&entries, now, &self.config);
        let expired: BTreeSet<_> = ops
            .iter()
            .filter_map(|op| match op {
                ChangeOp::Expire { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        let live: Vec<MemoryEntry> = entries
            .iter()
            .filter(|e| !expired.contains(&e.id))
            .cloned()
            .collect();
        let merges = rules::duplicates(&live, now, &self.config);
        let mut hidden = expired.clone();
        for op in &merges {
            if let ChangeOp::Merge { duplicates, .. } = op {
                hidden.extend(duplicates.iter().cloned());
            }
        }
        let contra = rules::contradictions(&live, &hidden, &journal, now);
        run.expired = expired.len();
        run.merged = merges.len();
        run.resolved = contra
            .iter()
            .filter(|op| matches!(op, ChangeOp::Resolve { .. }))
            .count();
        run.conflicts = contra.len() - run.resolved;
        ops.extend(merges);
        ops.extend(contra);
        if let Some(batch) = self.batch(scope, &live, now)
            && let Some(output) = self.ask_model(&batch, report).await
        {
            let (llm_ops, rejected) = rules::proposals_to_ops(&batch, &output, &live, &self.config);
            run.created = llm_ops
                .iter()
                .filter(|op| matches!(op, ChangeOp::Create { .. }))
                .count();
            run.consolidated = batch.episodes.len();
            run.rejected = rejected;
            ops.extend(llm_ops);
        }
        if !ops.is_empty() {
            let set = ChangeSet {
                scope: scope.clone(),
                run: report.run.clone(),
                ops,
            };
            memory.apply_changes(&Accessor::Guardian, &set)?;
        }
        Ok(run)
    }

    fn batch(
        &self,
        scope: &MemoryScope,
        live: &[MemoryEntry],
        now: DateTime<Utc>,
    ) -> Option<ConsolidationBatch> {
        let consolidator = self.ports.consolidator.as_ref()?;
        if self.config.auto_extract == AutoExtract::Off {
            return None;
        }
        let private = match scope {
            MemoryScope::Session(s) => self.ports.privacy.is_private(s),
            _ => false,
        };
        if private && self.config.require_local_for_private && !consolidator.model().local {
            return None;
        }
        let episodes: Vec<EpisodeView> = live
            .iter()
            .filter(|e| e.layer == Layer::Episodic && e.trusted && e.consolidated_at.is_none())
            .filter(|e| entry_state(e, now) == EntryState::Active && e.origin.derivation.is_none())
            .take(self.config.max_episodes_per_scope)
            .map(|e| EpisodeView {
                id: e.id.clone(),
                text: e.text.clone(),
                created_at: e.created_at,
            })
            .collect();
        if episodes.is_empty() {
            return None;
        }
        let known_facts = live
            .iter()
            .filter(|e| {
                e.layer == Layer::Semantic && e.trusted && entry_state(e, now) == EntryState::Active
            })
            .rev()
            .take(self.config.max_known_facts)
            .map(|e| FactView {
                id: e.id.clone(),
                text: e.text.clone(),
                subject: e.subject.clone(),
            })
            .collect();
        Some(ConsolidationBatch {
            scope: scope.clone(),
            private,
            episodes,
            known_facts,
        })
    }

    async fn ask_model(
        &self,
        batch: &ConsolidationBatch,
        report: &mut RunReport,
    ) -> Option<crate::ports::ConsolidatorOutput> {
        let consolidator = self.ports.consolidator.as_ref()?;
        if report.budget_denied {
            return None;
        }
        let model = consolidator.model();
        let estimate = consolidator.estimate_micro_usd(batch);
        if let BudgetVerdict::Deny { .. } = self.ports.budget.check(&model, estimate).await {
            report.budget_denied = true;
            return None;
        }
        report.llm_calls += 1;
        match consolidator.consolidate(batch).await {
            Ok(output) => {
                if let Some(usage) = &output.usage
                    && let Err(e) = self.ports.budget.record(usage).await
                {
                    report.errors.push(e.to_string());
                }
                Some(output)
            }
            Err(e) => {
                report.errors.push(e.to_string());
                None
            }
        }
    }
}
