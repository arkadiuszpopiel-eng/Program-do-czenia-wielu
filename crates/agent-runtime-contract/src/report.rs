//! Raport końcowy przebiegu (v1): co zrobiono, kroki cofalne („Cofnij”), weryfikacja, steering,
//! koszty — liczony wyłącznie z dziennika zdarzeń (ten sam dla `-impl` i `-fake`), z raportami
//! podprzebiegów (delegacje, Krytyczka). Pomocnik [`steps_before_delivery`] mierzy F5-02.

use core_bus_contract::RunId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tools_common_contract::UndoRef;

use crate::event::{RunEvent, RunEventEnvelope, RunOutcome, StepKind, StepStatus, UsageTotals};

/// Jedna linia kroku w raporcie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StepLine {
    /// Numer kroku.
    pub step: u32,
    /// Typ.
    pub kind: StepKind,
    /// Narzędzie.
    pub tool: Option<String>,
    /// Status.
    pub status: StepStatus,
    /// Skrót wejścia.
    pub input: String,
    /// Skrót wyjścia.
    pub output: String,
    /// Krok „Cofnij”.
    pub undo: Option<UndoRef>,
}

/// Wynik jednej weryfikacji (Krytyczka albo samoweryfikacja).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VerificationLine {
    /// Czy cel osiągnięty.
    pub ok: bool,
    /// Notatka (kto weryfikował i co znalazł).
    pub note: String,
}

/// Raport przebiegu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunReport {
    /// Przebieg.
    pub run: RunId,
    /// Cel (z `Started`).
    pub goal: String,
    /// Wynik (`None` = jeszcze trwa albo oddany schedulerowi).
    pub outcome: Option<RunOutcome>,
    /// Kroki narzędzi zakończone sukcesem (co zrobiono).
    pub done: Vec<StepLine>,
    /// Kroki narzędzi bez sukcesu (odmowy, błędy, anulowania).
    pub not_done: Vec<StepLine>,
    /// Kroki cofalne w kolejności wykonania (cofaj od końca).
    pub undoable: Vec<UndoRef>,
    /// Weryfikacje.
    pub verification: Vec<VerificationLine>,
    /// Przyjęte wiadomości sterujące.
    pub steering: Vec<String>,
    /// Przebieg widział niezaufaną treść.
    pub tainted: bool,
    /// Zużycie tego przebiegu (ostatnie `Usage`).
    pub usage: UsageTotals,
    /// Raporty podprzebiegów (delegacje, Krytyczka).
    pub children: Vec<RunReport>,
}

impl RunReport {
    /// Raport z dziennika zdarzeń (bez podprzebiegów).
    pub fn from_events(run: RunId, events: &[RunEventEnvelope]) -> Self {
        let mut r = Self {
            run,
            goal: String::new(),
            outcome: None,
            done: Vec::new(),
            not_done: Vec::new(),
            undoable: Vec::new(),
            verification: Vec::new(),
            steering: Vec::new(),
            tainted: false,
            usage: UsageTotals::default(),
            children: Vec::new(),
        };
        for env in events {
            match &env.event {
                RunEvent::Started { goal, .. } => r.goal.clone_from(goal),
                RunEvent::StepFinished {
                    step,
                    kind: kind @ StepKind::Tool,
                    tool,
                    status,
                    input,
                    output,
                    undo,
                    ..
                } => {
                    let line = StepLine {
                        step: *step,
                        kind: *kind,
                        tool: tool.clone(),
                        status: *status,
                        input: input.clone(),
                        output: output.clone(),
                        undo: undo.clone(),
                    };
                    if *status == StepStatus::Ok {
                        r.undoable.extend(undo.iter().cloned());
                        r.done.push(line);
                    } else {
                        r.not_done.push(line);
                    }
                }
                RunEvent::Verified { ok, note } => r.verification.push(VerificationLine {
                    ok: *ok,
                    note: note.clone(),
                }),
                RunEvent::Steered { message } => r.steering.push(message.clone()),
                RunEvent::Tainted { .. } => r.tainted = true,
                RunEvent::Usage(u) => r.usage = *u,
                RunEvent::Finished { outcome } => r.outcome = Some(outcome.clone()),
                _ => {}
            }
        }
        r
    }

    /// Zużycie łącznie z podprzebiegami (koszt całego zadania).
    pub fn total_usage(&self) -> UsageTotals {
        self.children
            .iter()
            .map(RunReport::total_usage)
            .fold(self.usage, |mut acc, c| {
                acc.input_tokens = acc.input_tokens.saturating_add(c.input_tokens);
                acc.output_tokens = acc.output_tokens.saturating_add(c.output_tokens);
                acc.cost_nano_usd = acc.cost_nano_usd.saturating_add(c.cost_nano_usd);
                acc.steps = acc.steps.saturating_add(c.steps);
                acc.tool_calls = acc.tool_calls.saturating_add(c.tool_calls);
                acc
            })
    }

    /// Wszystkie kroki cofalne (ten przebieg, potem podprzebiegi).
    pub fn all_undoable(&self) -> Vec<UndoRef> {
        let mut out = self.undoable.clone();
        for c in &self.children {
            out.extend(c.all_undoable());
        }
        out
    }

    /// Krótkie podsumowanie po polsku (dla Dyrygentki / Osi czasu).
    pub fn summary_pl(&self) -> String {
        let total = self.total_usage();
        let state = match &self.outcome {
            Some(RunOutcome::Completed {
                verified: Some(true),
                ..
            }) => "zakończone i zweryfikowane",
            Some(RunOutcome::Completed {
                verified: Some(false),
                ..
            }) => "zakończone, weryfikacja negatywna",
            Some(RunOutcome::Completed { .. }) => "zakończone",
            Some(RunOutcome::BudgetExceeded { .. }) => "przerwane: budżet",
            Some(RunOutcome::Cancelled) => "anulowane",
            Some(RunOutcome::LoopDetected { .. }) => "przerwane: pętla",
            Some(RunOutcome::Refused) => "odmowa modelu",
            Some(RunOutcome::Failed { .. }) => "błąd",
            None => "w toku",
        };
        format!(
            "Zadanie {state}: {} kroków wykonanych, {} niewykonanych, {} cofalnych, {} podzadań; \
             tokeny {}, koszt {} nUSD.",
            self.done.len(),
            self.not_done.len(),
            self.all_undoable().len(),
            self.children.len(),
            total.tokens(),
            total.cost_nano_usd
        )
    }
}

/// F5-02: ile kroków atomowych **rozpoczęło się** po wysłaniu sterowania (zdarzenia o numerze
/// większym niż `sent_after_seq`), zanim agentka je przyjęła (`Steered` z treścią `message`).
/// `None` = sterowanie nieprzyjęte. Próg: najwyżej 1 (krok w toku może się dokończyć).
pub fn steps_before_delivery(
    events: &[RunEventEnvelope],
    sent_after_seq: u64,
    message: &str,
) -> Option<u32> {
    let mut started = 0u32;
    for env in events.iter().filter(|e| e.seq > sent_after_seq) {
        match &env.event {
            RunEvent::StepStarted { .. } => started += 1,
            RunEvent::Steered { message: m } if m == message => return Some(started),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools_common_contract::UndoService;

    fn env(seq: u64, event: RunEvent) -> RunEventEnvelope {
        RunEventEnvelope {
            run: RunId::new("r"),
            seq,
            at_ms: seq,
            event,
        }
    }

    fn tool_step(step: u32, status: StepStatus, undo: bool) -> RunEvent {
        RunEvent::StepFinished {
            step,
            kind: StepKind::Tool,
            tool: Some("fs_write".into()),
            status,
            input: "a".into(),
            output: "b".into(),
            untrusted: false,
            undo: undo.then(|| UndoRef {
                service: UndoService::Journal,
                id: u64::from(step),
                text: "x".into(),
            }),
            intent: None,
            approval: None,
        }
    }

    #[test]
    fn report_from_events_and_children() {
        let events = vec![
            env(
                1,
                RunEvent::Started {
                    goal: "cel".into(),
                    tools: vec![],
                    budget: Default::default(),
                },
            ),
            env(2, tool_step(1, StepStatus::Ok, true)),
            env(3, tool_step(2, StepStatus::Denied, false)),
            env(
                4,
                RunEvent::Steered {
                    message: "stop".into(),
                },
            ),
            env(
                5,
                RunEvent::Verified {
                    ok: true,
                    note: "Gama".into(),
                },
            ),
            env(
                6,
                RunEvent::Usage(UsageTotals {
                    input_tokens: 10,
                    cost_nano_usd: 7,
                    ..Default::default()
                }),
            ),
            env(
                7,
                RunEvent::Tainted {
                    source: safety_broker_contract::TaintSource::File,
                },
            ),
            env(
                8,
                RunEvent::Finished {
                    outcome: RunOutcome::Completed {
                        summary: "ok".into(),
                        verified: Some(true),
                    },
                },
            ),
        ];
        let mut r = RunReport::from_events(RunId::new("r"), &events);
        assert_eq!(r.goal, "cel");
        assert_eq!(
            (r.done.len(), r.not_done.len(), r.undoable.len()),
            (1, 1, 1)
        );
        assert!(r.tainted && r.steering == ["stop"] && r.verification[0].ok);
        let child = r.clone();
        r.children.push(child);
        assert_eq!(r.total_usage().input_tokens, 20);
        assert_eq!(r.all_undoable().len(), 2);
        assert!(r.summary_pl().contains("zweryfikowane"));
        assert!(r.summary_pl().contains("1 podzadań"));
    }

    #[test]
    fn steering_latency_counts_started_steps() {
        let started = |seq, step| {
            env(
                seq,
                RunEvent::StepStarted {
                    step,
                    kind: StepKind::Tool,
                    tool: None,
                    input: String::new(),
                },
            )
        };
        let events = vec![
            started(1, 1),
            started(2, 2),
            env(
                3,
                RunEvent::Steered {
                    message: "m".into(),
                },
            ),
            started(4, 3),
        ];
        assert_eq!(steps_before_delivery(&events, 0, "m"), Some(2));
        assert_eq!(steps_before_delivery(&events, 1, "m"), Some(1));
        assert_eq!(steps_before_delivery(&events, 2, "m"), Some(0));
        assert_eq!(steps_before_delivery(&events, 3, "m"), None);
        assert_eq!(steps_before_delivery(&events, 0, "x"), None);
    }
}
