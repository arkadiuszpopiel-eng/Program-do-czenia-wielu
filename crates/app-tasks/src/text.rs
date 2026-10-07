//! Opisy po polsku dla UI: powody czekania zadań, przyczyny wyzwoleń, warunki i efekty reguł
//! Marszałka („podgląd zawężenia").

use marshal_contract::{Effect, EffectivePolicy, PauseScope, Rule};
use safety_broker_contract::Capability;
use scheduler_contract::{BlockReason, CancelCause, ExpiryReason, Termination};
use triggers_contract::{FireCause, RunOutcome, SuppressReason};

fn name<T: serde::Serialize>(x: &T) -> String {
    match serde_json::to_value(x) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(v) => v.to_string(),
        Err(_) => "?".into(),
    }
}

fn hhmm(min: u16) -> String {
    format!("{:02}:{:02}", min / 60, min % 60)
}

/// Powód czekania zadania.
pub fn block(b: &BlockReason) -> String {
    match b {
        BlockReason::Dependencies => "czeka na poprzednie zadania".into(),
        BlockReason::NotBefore { .. } => "czeka na zaplanowaną godzinę".into(),
        BlockReason::NotIdle => "czeka, aż przestaniesz pracować przy komputerze".into(),
        BlockReason::GameMode => "wstrzymane w trybie gry".into(),
        BlockReason::NoAgent => "brak agentki z wymaganą rolą w obsadzie".into(),
        BlockReason::AgentBusy => "agentka jest zajęta innym zadaniem".into(),
        BlockReason::Concurrency => "limit zadań równoległych".into(),
        BlockReason::Resources { busy } => format!(
            "zasoby zajęte: {}",
            busy.iter().map(name).collect::<Vec<_>>().join(", ")
        ),
        BlockReason::Reserved { .. } => "zasoby zarezerwowane dla ważniejszego zadania".into(),
        BlockReason::Paused => "wstrzymane przez Ciebie".into(),
        BlockReason::Backoff { .. } => "odstęp przed ponowieniem".into(),
    }
}

/// Opis zakończenia (gdy nie sukces).
pub fn termination(t: &Termination) -> Option<String> {
    Some(match t {
        Termination::Succeeded { .. } => return None,
        Termination::Failed { error, attempts } => format!("{error} (prób: {attempts})"),
        Termination::Cancelled { cause } => match cause {
            CancelCause::User { reason } if !reason.is_empty() => format!("anulowane: {reason}"),
            CancelCause::User { .. } => "anulowane przez Ciebie".into(),
            CancelCause::Ancestor { root } => format!("anulowane razem z zadaniem {root}"),
            CancelCause::KillSwitch => "STOP WSZYSTKIEGO".into(),
            CancelCause::Worker => "przerwane przez wykonawczynię".into(),
        },
        Termination::Skipped { dependency, .. } => {
            format!("pominięte — warunek zadania {dependency} niespełniony")
        }
        Termination::Expired { reason } => match reason {
            ExpiryReason::NotStarted { blocked: Some(b) } => {
                format!("termin minął przed startem ({})", block(b))
            }
            ExpiryReason::NotStarted { blocked: None } => "termin minął przed startem".into(),
            ExpiryReason::WhileRunning => "termin minął w trakcie".into(),
        },
        Termination::BudgetExceeded { budget } => {
            format!("przekroczony budżet {}", budget.label_pl())
        }
        Termination::BudgetBlocked { reason } => format!("budżet tła: {reason}"),
    })
}

/// Przyczyna wyzwolenia.
pub fn cause(c: &FireCause) -> String {
    match c {
        FireCause::Time { missed: 0, .. } => "harmonogram".into(),
        FireCause::Time { missed, .. } => format!("harmonogram (zaległe: {missed})"),
        FireCause::Manual { .. } => "ręcznie".into(),
        FireCause::File { path } => {
            let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
            format!("nowy plik: {file}")
        }
        FireCause::Message { .. } => "nowa wiadomość".into(),
        FireCause::TaskFinished { task, result, .. } => format!("koniec zadania {task} ({result})"),
        FireCause::Deferred { original } => format!("odłożone: {}", cause(original)),
    }
}

/// Wynik wyzwolenia: (rodzaj, zadanie, szczegóły).
pub fn outcome(o: &RunOutcome) -> (&'static str, Option<String>, Option<String>) {
    match o {
        RunOutcome::Submitted { task } => ("submitted", Some(task.to_string()), None),
        RunOutcome::Suppressed { reason } => ("suppressed", None, Some(suppressed(*reason).into())),
        RunOutcome::Deferred { .. } => ("deferred", None, Some("odłożone do końca ciszy".into())),
        RunOutcome::Failed { error } => ("failed", None, Some(error.clone())),
    }
}

fn suppressed(r: SuppressReason) -> &'static str {
    match r {
        SuppressReason::RateLimited => "limit uruchomień wyzwalacza",
        SuppressReason::GlobalRateLimited => "globalny limit wyzwalaczy",
        SuppressReason::Quiet => "okno ciszy",
        SuppressReason::Dnd => "„nie przeszkadzać”",
        SuppressReason::ChainTooDeep => "za długi łańcuch wyzwalaczy",
        SuppressReason::SelfLoop => "wyzwalacz reagowałby na własne zadanie",
        SuppressReason::Missed => "przegapione (pominięte)",
    }
}

/// Zdolność w skrócie (`fs.write c:\…\**`).
pub fn capability(c: &Capability) -> String {
    if let Some(scope) = c.path_scope() {
        let tail = if scope.subtree() { "\\**" } else { "" };
        return format!("{} {}{tail}", c.family(), scope.canonical());
    }
    let v = serde_json::to_value(c).unwrap_or_default();
    format!("{} {}", c.family(), name(&v["scope"]))
}

/// Warunek reguły.
pub fn when(rule: &Rule) -> Vec<String> {
    let w = &rule.when;
    let mut out = Vec::new();
    if let Some(r) = &w.resource {
        out.push(format!("zasób: {}", name(r)));
    }
    if w.event.is_some() {
        out.push("gdy mówisz".into());
    }
    if let Some(a) = &w.agent {
        out.push(format!("agentka: {a}"));
    }
    if let Some(r) = &w.role {
        out.push(format!("rola: {r}"));
    }
    if let Some(c) = &w.task_class {
        out.push(format!("klasa zadań: {}", name(c)));
    }
    if let Some(o) = &w.origin {
        out.push(format!("pochodzenie: {}", name(o)));
    }
    if w.tainted == Some(true) {
        out.push("zadania z treścią niezaufaną".into());
    }
    if let Some(t) = &w.time {
        out.push(format!("w godz. {}–{}", hhmm(t.start_min), hhmm(t.end_min)));
    }
    if out.is_empty() {
        out.push("zawsze".into());
    }
    out
}

/// Efekt reguły (zawsze zawężenie).
pub fn effect(e: &Effect) -> String {
    match e {
        Effect::Exclusive {
            resource,
            max_wait_ms,
            on_timeout,
        } => format!(
            "wyłączny dostęp do zasobu {} (czekanie ≤ {} s, potem: {})",
            name(resource),
            max_wait_ms / 1000,
            name(on_timeout)
        ),
        Effect::Preempt { classes } => format!(
            "gdy mówisz, wstrzymaj: {}",
            classes.iter().map(name).collect::<Vec<_>>().join(", ")
        ),
        Effect::PauseAtAtomic { scope, .. } => match scope {
            PauseScope::Gui => "pauza zadań GUI w punktach atomowych".into(),
            PauseScope::Audio => "pauza zadań audio w punktach atomowych".into(),
        },
        Effect::DenyCapability { capability: c } => format!("zakaz: {}", capability(c)),
        Effect::DenyFamily { family } => format!("zakaz całej rodziny {family}"),
        Effect::RestrictTo { capabilities } => format!(
            "tylko: {}",
            capabilities
                .iter()
                .map(capability)
                .collect::<Vec<_>>()
                .join("; ")
        ),
        Effect::RequireApproval { family } => format!("{family} — zawsze z zatwierdzeniem"),
        Effect::CapAutonomy { max } => format!("autonomia najwyżej {}", name(max)),
        Effect::CapBudget {
            max_steps,
            max_wall_ms,
            max_cost_micro_pln,
        } => {
            let mut parts = Vec::new();
            if let Some(s) = max_steps {
                parts.push(format!("{s} kroków"));
            }
            if let Some(ms) = max_wall_ms {
                parts.push(format!("{} min", ms / 60_000));
            }
            if let Some(c) = max_cost_micro_pln {
                parts.push(format!("{:.2} zł", *c as f64 / 1_000_000.0));
            }
            format!("budżet zadania najwyżej: {}", parts.join(", "))
        }
        Effect::MaxParallel { n } => format!("najwyżej {n} zadań naraz"),
        Effect::QuietHours { start_min, end_min } => {
            format!("cisza wyzwalaczy {}–{}", hhmm(*start_min), hhmm(*end_min))
        }
        Effect::DenyBridges => "bez mostów CLI".into(),
    }
}

/// Polityka efektywna w punktach.
pub fn policy(p: &EffectivePolicy) -> Vec<String> {
    let mut out = vec![
        format!("autonomia najwyżej {}", name(&p.autonomy)),
        format!(
            "zadanie: ≤ {} kroków, ≤ {} min",
            p.max_steps,
            p.max_wall_ms / 60_000
        ),
        format!("najwyżej {} zadań naraz", p.max_parallel),
    ];
    for f in &p.denied_families {
        out.push(format!("zakaz rodziny {f}"));
    }
    for c in &p.denied {
        out.push(format!("zakaz: {}", capability(c)));
    }
    for f in &p.approval_families {
        out.push(format!("{f} — z zatwierdzeniem"));
    }
    if p.bridges_denied {
        out.push("bez mostów CLI".into());
    }
    for (s, e) in &p.quiet {
        out.push(format!("cisza wyzwalaczy {}–{}", hhmm(*s), hhmm(*e)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_and_conditions_read_as_narrowing() {
        let rule: Rule = marshal_contract::parse_rule(&serde_json::json!({
            "id": "bez-shella", "when": { "agent": "delta", "time": { "start_min": 1320, "end_min": 360 } },
            "then": [{ "effect": "deny_family", "family": "shell.exec" }, { "effect": "max_parallel", "n": 2 }]
        }))
        .unwrap();
        assert_eq!(when(&rule), vec!["agentka: delta", "w godz. 22:00–06:00"]);
        assert_eq!(effect(&rule.then[0]), "zakaz całej rodziny shell.exec");
        assert_eq!(effect(&rule.then[1]), "najwyżej 2 zadań naraz");
        assert_eq!(block(&BlockReason::Paused), "wstrzymane przez Ciebie");
        assert!(
            termination(&Termination::Cancelled {
                cause: CancelCause::KillSwitch
            })
            .is_some_and(|t| t.contains("STOP"))
        );
        assert_eq!(
            cause(&FireCause::File {
                path: "C:\\a\\b.pdf".into()
            }),
            "nowy plik: b.pdf"
        );
    }
}
