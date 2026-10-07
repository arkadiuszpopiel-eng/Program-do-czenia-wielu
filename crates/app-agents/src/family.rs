//! Rodzina przebiegu: przebieg główny + podprzebiegi (delegacja `delegate_task`, Krytyczka,
//! umiejętność) z jednego runtime. [`RunFamily`] scala dzienniki na żywo (bez luk — każdy przez
//! [`RunFeed`]), wykrywa nowe podprzebiegi (`AgentRuntime::children`) po każdym zdarzeniu i co
//! 250 ms; [`FamilyProjector`] rzutuje każdy przebieg osobno na Replay — podprzebieg ma rodzica
//! i etykietę, nie dopisuje kroków do tury czatu (`turn_id = None`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use agent_runtime_contract::{
    AgentRuntime, CheckpointStore, MemCheckpointStore, RunError, RunEvent, RunEventEnvelope, RunId,
};
use agent_runtime_impl::Runtime;
use app_api::dto::AgentRun;
use futures_util::FutureExt;
use futures_util::future::{BoxFuture, select_all};

use crate::project::{Projection, RunContext, RunProjector};
use crate::runner::RunFeed;
use crate::tickets::TicketLog;

/// Co wiadomo o podprzebiegu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildInfo {
    /// Rodzic.
    pub parent: RunId,
    /// Agentka (z checkpointu; `None` — jeszcze nie zapisany).
    pub agent: Option<String>,
    /// Etykieta (`RunOptions::label`) albo rola Krytyczki/delegacji.
    pub label: Option<String>,
}

/// Dzienniki przebiegu głównego i podprzebiegów na żywo.
pub struct RunFamily {
    runtime: Arc<Runtime>,
    store: MemCheckpointStore,
    feeds: Vec<(RunId, RunFeed)>,
    parents: BTreeMap<RunId, RunId>,
    known: BTreeSet<RunId>,
}

enum Next {
    Event(usize, Option<Box<RunEventEnvelope>>),
    Tick,
}

impl RunFamily {
    /// Rodzina istniejącego przebiegu (dziennik od początku, potem na żywo).
    pub fn attach(
        runtime: Arc<Runtime>,
        store: MemCheckpointStore,
        main: RunId,
    ) -> Result<Self, RunError> {
        let feed = RunFeed::attach(runtime.clone(), main.clone())?;
        let mut known = BTreeSet::new();
        known.insert(main.clone());
        Ok(Self {
            runtime,
            store,
            feeds: vec![(main, feed)],
            parents: BTreeMap::new(),
            known,
        })
    }

    /// Dołącza podprzebiegi, które pojawiły się od ostatniego sprawdzenia (rekurencyjnie).
    fn discover(&mut self) {
        let mut frontier: Vec<RunId> = self.known.iter().cloned().collect();
        while let Some(run) = frontier.pop() {
            for child in self.runtime.children(&run).unwrap_or_default() {
                if self.known.contains(&child) {
                    continue;
                }
                if let Ok(feed) = RunFeed::attach(self.runtime.clone(), child.clone()) {
                    self.known.insert(child.clone());
                    self.parents.insert(child.clone(), run.clone());
                    self.feeds.push((child.clone(), feed));
                    frontier.push(child);
                }
            }
        }
    }

    /// Podprzebieg: rodzic, agentka, etykieta (`None` — przebieg główny albo nieznany).
    pub fn child_info(&self, run: &RunId) -> Option<ChildInfo> {
        let parent = self.parents.get(run)?.clone();
        let cp = self.store.latest(run).ok().flatten();
        Some(ChildInfo {
            parent,
            agent: cp.as_ref().map(|c| c.spec.agent.to_string()),
            label: cp.and_then(|c| c.options.label),
        })
    }

    /// Następne zdarzenie dowolnego przebiegu rodziny; `None`, gdy wszystkie się zakończyły.
    pub async fn next(&mut self) -> Option<RunEventEnvelope> {
        loop {
            self.discover();
            if self.feeds.is_empty() {
                return None;
            }
            let next = {
                let mut futures: Vec<BoxFuture<'_, Next>> = self
                    .feeds
                    .iter_mut()
                    .enumerate()
                    .map(|(i, (_, feed))| {
                        feed.next()
                            .map(move |e| Next::Event(i, e.map(Box::new)))
                            .boxed()
                    })
                    .collect();
                futures.push(
                    tokio::time::sleep(Duration::from_millis(250))
                        .map(|()| Next::Tick)
                        .boxed(),
                );
                select_all(futures).await.0
            };
            match next {
                Next::Tick => {}
                Next::Event(_, Some(env)) => return Some(*env),
                Next::Event(i, None) => {
                    self.feeds.remove(i);
                }
            }
        }
    }
}

/// Projekcja rodziny: projektor przebiegu głównego + projektory podprzebiegów.
pub struct FamilyProjector {
    main_run: String,
    base: RunContext,
    titles: BTreeMap<String, String>,
    tickets: Option<Arc<TicketLog>>,
    main: RunProjector,
    children: BTreeMap<RunId, (RunProjector, bool)>,
}

impl FamilyProjector {
    /// Projekcja z kontekstem przebiegu głównego.
    pub fn new(
        ctx: RunContext,
        titles: BTreeMap<String, String>,
        tickets: Option<Arc<TicketLog>>,
    ) -> Self {
        let main = RunProjector::new(ctx.clone(), titles.clone(), tickets.clone());
        Self {
            main_run: ctx.run.clone(),
            base: ctx,
            titles,
            tickets,
            main,
            children: BTreeMap::new(),
        }
    }

    /// Projektor przebiegu głównego (stan tury, wynik).
    pub fn main(&self) -> &RunProjector {
        &self.main
    }

    /// Stosuje zdarzenie; zwraca nagłówek przebiegu, którego dotyczy, i jego projekcję.
    pub fn apply(
        &mut self,
        family: &RunFamily,
        env: &RunEventEnvelope,
    ) -> (AgentRun, Projection, bool) {
        if env.run.as_str() == self.main_run {
            let p = self.main.apply(env);
            return (self.main.run().clone(), p, true);
        }
        let info = family.child_info(&env.run);
        let entry = self.children.entry(env.run.clone()).or_insert_with(|| {
            let mut ctx = self.base.clone();
            ctx.run = env.run.as_str().to_owned();
            ctx.turn_id = None;
            if let RunEvent::Started { goal, .. } = &env.event {
                ctx.goal.clone_from(goal);
            }
            let parent = info
                .as_ref()
                .map_or_else(|| self.main_run.clone(), |i| i.parent.as_str().to_owned());
            let parent = app_api::ids::run_dto(&ctx.session, &parent);
            let label = info.as_ref().and_then(|i| i.label.clone());
            let projector = RunProjector::new(ctx, self.titles.clone(), self.tickets.clone())
                .with_parent(parent, label);
            (projector, false)
        });
        if !entry.1
            && let Some(agent) = info.and_then(|i| i.agent)
        {
            entry.0.set_agent(&agent);
            entry.1 = true;
        }
        let p = entry.0.apply(env);
        (entry.0.run().clone(), p, false)
    }
}
