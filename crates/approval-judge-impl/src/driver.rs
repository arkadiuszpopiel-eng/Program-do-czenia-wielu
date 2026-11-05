//! Kierowca sędziego w jądrze: pyta Brokera o prośby „czeka na sędziego”, składa wejście z kontekstu rozmowy,
//! woła sędziego i odsyła werdykt. Pytanie o kolejkę jest też znakiem życia — bez kierowcy Broker od razu
//! pokazuje kartę właściciela.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use approval_judge_contract::{
    ApprovalJudge, JudgeContext, JudgeContextSource, JudgeInput, JudgeVerdict, JudgedAction,
};
use providers_contract::CancellationToken;
use safety_broker_contract::{Broker, BrokerError, JudgeRequest, JudgeResolution};

/// Co ile kierowca pyta o kolejkę (znacznie poniżej progu „kierowca nieobecny” w Brokerze).
pub const POLL_EVERY: Duration = Duration::from_millis(1_000);

/// Sędziowie do wyboru: lokalny (dla przebiegów lokalnych i nieznanych) i dowolny (Router, szczebel
/// sprawdzania — dla przebiegów w chmurze).
#[derive(Clone, Default)]
pub struct Judges {
    /// Sędzia na modelu lokalnym.
    pub local: Option<Arc<dyn ApprovalJudge>>,
    /// Sędzia na dowolnym modelu z Routera.
    pub any: Option<Arc<dyn ApprovalJudge>>,
}

impl Judges {
    /// Sędzia dla kontekstu: przebieg lokalny → tylko lokalny.
    pub fn pick(&self, local_only: bool) -> Option<Arc<dyn ApprovalJudge>> {
        if local_only {
            self.local.clone()
        } else {
            self.any.clone().or_else(|| self.local.clone())
        }
    }
}

/// Wejście sędziego z prośby Brokera i kontekstu rozmowy.
pub fn build_input(req: &JudgeRequest, ctx: JudgeContext) -> JudgeInput {
    let mut capability_summary = req
        .summary
        .clone()
        .unwrap_or_else(|| format!("{:?}", req.capability));
    if let Some(w) = &req.workdir {
        capability_summary.push_str(&format!(" (folder pracy: {w})"));
    }
    JudgeInput {
        goal: ctx.goal,
        transcript: ctx.transcript,
        action: JudgedAction {
            tool: req.tool.clone(),
            capability_summary,
            command: req.command.clone(),
            paths: req.workdir.iter().cloned().collect(),
            host: req.host.clone(),
            reversible: req.reversible,
            destructive: req.destructive,
            rules: req.rules.clone(),
            outside_workdir: req.outside_workdir,
            tainted: req.tainted,
        },
        limits: ctx.limits,
    }
}

/// Kierowca sędziego.
pub struct JudgeDriver {
    broker: Arc<dyn Broker>,
    judges: Judges,
    context: Arc<dyn JudgeContextSource>,
    in_flight: Mutex<BTreeSet<u64>>,
}

impl JudgeDriver {
    /// Nowy kierowca (Broker z rolą `Core`).
    pub fn new(
        broker: Arc<dyn Broker>,
        judges: Judges,
        context: Arc<dyn JudgeContextSource>,
    ) -> Arc<Self> {
        Arc::new(Self {
            broker,
            judges,
            context,
            in_flight: Mutex::new(BTreeSet::new()),
        })
    }

    fn flights(&self) -> MutexGuard<'_, BTreeSet<u64>> {
        self.in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Jedna ocena (bez współbieżności — testy i `tick`).
    pub async fn judge_one(&self, req: JudgeRequest) -> Result<(), BrokerError> {
        let started = Instant::now();
        let ctx = self.context.context(
            req.holder.session.as_str(),
            req.holder.agent.as_ref().map(|a| a.as_str()),
        );
        let (verdict, model, task) = match ctx {
            None => (
                JudgeVerdict::escalate("brak zapisu rozmowy dla sędziego"),
                "brak".to_owned(),
                None,
            ),
            Some(ctx) => {
                let task = ctx.task.clone();
                match self.judges.pick(ctx.local_only) {
                    None => (
                        JudgeVerdict::escalate("brak modelu sędziego (lokalnego dla tej rozmowy)"),
                        "brak".to_owned(),
                        task,
                    ),
                    Some(judge) => {
                        let input = build_input(&req, ctx);
                        (judge.review(&input).await, judge.model_name(), task)
                    }
                }
            }
        };
        let took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let resolution = JudgeResolution {
            verdict,
            model,
            task,
            took_ms,
        };
        self.broker.judge_resolve(req.approval, resolution).await
    }

    /// Pobiera kolejkę i uruchamia ocenę każdej nowej prośby we własnym zadaniu. Zwraca liczbę nowych.
    pub fn tick(self: &Arc<Self>) -> usize {
        let queue = self.broker.judge_queue();
        let mut started = 0;
        for req in queue {
            let id = req.approval.0;
            if !self.flights().insert(id) {
                continue;
            }
            started += 1;
            let me = Arc::clone(self);
            tokio::spawn(async move {
                if let Err(e) = me.judge_one(req).await {
                    // Termin minął albo Sędzia nieosiągalny — prośba i tak trafi do właściciela.
                    tracing::debug!(error = %e, approval = id, "werdykt sędziego nieprzyjęty");
                }
                me.flights().remove(&id);
            });
        }
        started
    }

    /// Pętla kierowcy do anulowania.
    pub async fn run(self: Arc<Self>, cancel: CancellationToken) {
        loop {
            self.tick();
            tokio::select! {
                () = cancel.cancelled() => return,
                () = tokio::time::sleep(POLL_EVERY) => {}
            }
        }
    }
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
