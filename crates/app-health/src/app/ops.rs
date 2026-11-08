//! Ulepszacz i evale „Zdrowia systemu" (komendy `improver_*`, `evals_*`) — część `HealthApp`
//! wydzielona z `app.rs` (limit 400 linii na plik).

use super::*;

impl HealthApp {
    /// `improver_list`.
    pub fn improver_view(&self) -> Result<ImproverView, AppError> {
        let imp = self.improver()?;
        let mut proposals = imp.proposals();
        proposals.sort_by_key(|p| std::cmp::Reverse(p.created_ms));
        Ok(ImproverView {
            proposals: proposals.iter().map(view::proposal).collect(),
            blocked: imp
                .blocked()
                .iter()
                .rev()
                .take(50)
                .map(view::blocked)
                .collect(),
            issues: imp.issue_drafts().iter().map(view::issue).collect(),
            idle_cycle: self.idle_cycle.load(std::sync::atomic::Ordering::Relaxed),
            last_cycle: lock(&self.last_cycle).clone(),
        })
    }

    /// Cykl Ulepszacza (ręcznie albo w bezczynności).
    pub(super) async fn run_cycle(&self, conditions: RunConditions) -> Result<(), AppError> {
        let snapshot = MetricsSnapshot {
            ts_ms: u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0),
            metrics: std::collections::BTreeMap::new(),
            observations: Vec::new(),
        };
        let result = self.improver()?.cycle(&snapshot, conditions).await;
        *lock(&self.last_cycle) = Some(iso(chrono::Utc::now()));
        result.map(|_| ()).map_err(improver_error)
    }

    /// `improver_cycle`: „Przeanalizuj teraz" — uruchomienie ręczne jest zgodą użytkownika (jak
    /// porządkowanie pamięci teraz), więc nie czeka na bezczynność.
    pub async fn cycle(&self) -> Result<ImproverView, AppError> {
        let conditions = RunConditions {
            on_battery: false,
            game_mode: false,
            user_idle: true,
        };
        self.run_cycle(conditions).await?;
        self.improver_view()
    }

    /// `improver_approve`: dokładnie przejrzany diff (`digest`), tylko R0 bez podpisu.
    pub async fn improver_approve(&self, id: u64, digest: &str) -> Result<ImproverView, AppError> {
        let approval = UserApproval {
            proposal: ProposalId(id),
            digest: digest.trim().to_owned(),
            surface: SURFACE.into(),
            signature: None,
        };
        self.improver()?
            .approve(approval)
            .await
            .map_err(improver_error)?;
        self.improver_view()
    }

    /// `improver_reject`.
    pub async fn improver_reject(&self, id: u64) -> Result<ImproverView, AppError> {
        self.improver()?
            .reject(ProposalId(id))
            .await
            .map_err(improver_error)?;
        self.improver_view()
    }

    /// `improver_rollback`.
    pub async fn improver_rollback(&self, id: u64) -> Result<ImproverView, AppError> {
        self.improver()?
            .rollback(ProposalId(id))
            .await
            .map_err(improver_error)?;
        self.improver_view()
    }

    pub(super) fn suite_view(
        catalog: &DirCatalog,
        suite: &SuiteId,
    ) -> Result<EvalSuiteView, AppError> {
        let manifest = catalog
            .manifest(suite)
            .map_err(|e| AppError::not_found(format!("Zestaw: {e}")))?;
        let (ok, problems) = match catalog.verify(suite) {
            Ok(r) => {
                let mut p: Vec<String> = r
                    .mismatched
                    .iter()
                    .map(|m| format!("zmieniony: {}", m.path))
                    .collect();
                p.extend(r.missing.iter().map(|m| format!("brak: {m}")));
                (p.is_empty(), p)
            }
            Err(e) => (false, vec![e.to_string()]),
        };
        Ok(EvalSuiteView {
            id: suite.to_string(),
            wave: manifest.wave.clone(),
            version: manifest.version.to_string(),
            status: format!("{:?}", manifest.status).to_lowercase(),
            integrity_ok: ok,
            problems,
            thresholds: u32::try_from(manifest.thresholds.len()).unwrap_or(u32::MAX),
        })
    }

    /// `evals_list`: zestawy (z integralnością) i ostatnie werdykty bramki.
    pub fn evals_view(&self) -> EvalsView {
        match &self.evals {
            Ok((_, catalog, gate)) => EvalsView {
                available: true,
                reason: None,
                suites: catalog
                    .suites()
                    .iter()
                    .filter_map(|s| Self::suite_view(catalog, &s.suite).ok())
                    .collect(),
                holdout_suites: u32::try_from(gate.holdout_suites().len()).unwrap_or(u32::MAX),
                verdicts: lock(&self.verdicts).iter().cloned().collect(),
            },
            Err(e) => EvalsView {
                available: false,
                reason: Some(e.clone()),
                suites: Vec::new(),
                holdout_suites: 0,
                verdicts: Vec::new(),
            },
        }
    }

    /// `evals_verify`: integralność jednego zestawu.
    pub fn verify(&self, suite: &str) -> Result<EvalSuiteView, AppError> {
        let (_, catalog, _) = self
            .evals
            .as_ref()
            .map_err(|_| AppError::unavailable("Evale", "evals"))?;
        let id = SuiteId::new(suite).map_err(|e| AppError::invalid(format!("Zestaw: {e}")))?;
        Self::suite_view(catalog, &id)
    }
}
