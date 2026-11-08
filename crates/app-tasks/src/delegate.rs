//! Delegacja z czatu („Delta, zleć to Claude Code"): zadanie mostu od użytkownika (pochodzenie
//! `User` — jedyne, dla którego most startuje bez zgody na harmonogram) w sesji czatu; tura
//! agentki czeka na wynik, a jej anulowanie (Esc, STOP) anuluje zadanie mostu.

use std::future::Future;

use agent_backends_contract::BridgeKind;
use core_bus_contract::SessionId;
use personas_contract::PersonaId;
use scheduler_contract::{
    Assignee, ExecutorKind, Scheduler, TaskClass, TaskId, TaskOrigin, TaskSpec, Termination,
};

use crate::app::TasksApp;
use crate::exec_bridge::UNVERIFIED_NOTE;

/// Wynik delegacji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delegated {
    /// Zadanie w schedulerze (panel Zadania, Replay).
    pub task: String,
    /// Tekst wyniku (z dopiskiem „niezweryfikowany przez Alfę") albo powód porażki.
    pub result: Result<String, String>,
}

fn describe(t: &Termination) -> Result<String, String> {
    match t {
        Termination::Succeeded { output } => Ok(output.summary.clone()),
        Termination::Failed { error, .. } => Err(error.clone()),
        Termination::Cancelled { .. } => Err("Zadanie mostu anulowano.".into()),
        other => Err(format!("Zadanie mostu nie zakończyło się: {other:?}")),
    }
}

impl TasksApp {
    /// Zleca polecenie mostowi CLI w imieniu użytkownika i czeka na wynik (albo `cancelled`).
    pub async fn delegate(
        &self,
        session: &SessionId,
        agent: &str,
        kind: BridgeKind,
        goal: &str,
        cancelled: impl Future<Output = ()> + Send,
    ) -> Delegated {
        let id = TaskId::new(self.next_id("d"));
        let goal = goal.trim();
        let title: String = goal.chars().take(60).collect();
        let persona = PersonaId::new(agent);
        let in_roster = self
            .p
            .roster
            .roster()
            .agents
            .iter()
            .any(|a| a.persona == persona);
        let assignee = if in_roster {
            Assignee::Persona(persona)
        } else {
            Assignee::AnyAgent
        };
        let mut spec = TaskSpec::new(
            id.clone(),
            title,
            assignee,
            TaskClass::User,
            TaskOrigin::User,
        );
        spec.executor = ExecutorKind::Bridge(kind);
        spec.session = Some(session.clone());
        spec.payload = serde_json::json!({ "goal": goal });
        // Jedna próba: ponowienie mostu z czatu to decyzja użytkownika („Ponów" w panelu).
        spec.retry.max_attempts = 1;
        let task = id.to_string();
        let fail = |e: String| Delegated {
            task: task.clone(),
            result: Err(e),
        };
        if goal.is_empty() {
            return fail("Brak polecenia dla mostu — napisz, co ma zrobić.".into());
        }
        if let Err(e) = self.p.scheduler.submit(vec![spec]) {
            return fail(format!("Scheduler odrzucił zadanie mostu: {e}"));
        }
        tokio::select! {
            waited = self.p.scheduler.wait(&id) => match waited {
                Ok(t) => Delegated { task: task.clone(), result: describe(&t) },
                Err(e) => fail(e.to_string()),
            },
            () = cancelled => {
                let _ = self.p.scheduler.cancel(&id, "anulowane przez użytkownika");
                fail("Zatrzymano — zadanie mostu anulowane.".into())
            }
        }
    }
}

/// Czy tekst wyniku pochodzi z mostu (dopisek dodaje wykonawczyni).
pub fn is_unverified(text: &str) -> bool {
    text.ends_with(UNVERIFIED_NOTE)
}
