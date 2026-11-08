//! DAG: przyjęcie zgłoszenia (walidacja, unikalność, zależności, cykle), rozstrzyganie
//! warunków zależności i poddrzewo delegacji.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::json;

use crate::engine::{Ctx, TaskRec};
use crate::error::TaskError;
use crate::events::{EVENT_SUBMITTED, short_title};
use crate::host::SchedHost;
use crate::ids::TaskId;
use crate::spec::{DepCondition, TaskSpec};
use crate::state::{BlockReason, TaskState, Termination};
use crate::validate::{MAX_ACTIVE_TASKS, MAX_TASKS_PER_SUBMIT, effective_deadline, validate_spec};

/// Stan zależności zadania.
pub(crate) enum DepsStatus {
    /// Wszystkie warunki spełnione.
    Met,
    /// Któraś poprzedniczka jeszcze trwa.
    Waiting,
    /// Warunek nie może być spełniony.
    Impossible(TaskId, DepCondition),
}

/// Czy zakończenie poprzedniczki spełnia warunek.
pub(crate) fn condition_met(condition: &DepCondition, done: &Termination) -> bool {
    match condition {
        DepCondition::Succeeded => done.is_success(),
        DepCondition::Failed => done.is_failure(),
        DepCondition::Finished => true,
        DepCondition::OutputEquals { key, value } => match done {
            Termination::Succeeded { output } => output.values.get(key) == Some(value),
            _ => false,
        },
    }
}

/// Cykl w grafie zgłoszenia (krawędzie: zależność → zadanie, rodzic → dziecko).
fn find_cycle(batch: &[TaskSpec]) -> Option<Vec<TaskId>> {
    let ids: BTreeSet<&TaskId> = batch.iter().map(|s| &s.id).collect();
    let mut edges: BTreeMap<&TaskId, Vec<&TaskId>> = BTreeMap::new();
    for spec in batch {
        let preds = spec
            .deps
            .iter()
            .map(|d| &d.task)
            .chain(spec.parent.as_ref())
            .filter(|p| ids.contains(p));
        for pred in preds {
            edges.entry(pred).or_default().push(&spec.id);
        }
    }
    // DFS z kolorami: 1 = na ścieżce, 2 = przetworzony.
    let mut color: BTreeMap<&TaskId, u8> = BTreeMap::new();
    for start in &ids {
        if color.contains_key(start) {
            continue;
        }
        let mut path: Vec<&TaskId> = vec![start];
        let mut stack: Vec<(&TaskId, usize)> = vec![(start, 0)];
        color.insert(start, 1);
        while let Some((node, idx)) = stack.pop() {
            let next = edges.get(node).and_then(|v| v.get(idx)).copied();
            match next {
                Some(succ) => {
                    stack.push((node, idx + 1));
                    match color.get(succ) {
                        Some(1) => {
                            let pos = path.iter().position(|p| *p == succ).unwrap_or(0);
                            let mut cycle: Vec<TaskId> =
                                path[pos..].iter().map(|t| (*t).clone()).collect();
                            cycle.push(succ.clone());
                            return Some(cycle);
                        }
                        Some(_) => {}
                        None => {
                            color.insert(succ, 1);
                            path.push(succ);
                            stack.push((succ, 0));
                        }
                    }
                }
                None => {
                    color.insert(node, 2);
                    path.pop();
                }
            }
        }
    }
    None
}

impl<H: SchedHost> Ctx<'_, H> {
    /// Przyjmuje zgłoszenie w całości albo wcale.
    pub(crate) fn submit_batch(&mut self, specs: Vec<TaskSpec>) -> Result<Vec<TaskId>, TaskError> {
        if specs.len() > MAX_TASKS_PER_SUBMIT {
            return Err(TaskError::Capacity {
                limit: MAX_TASKS_PER_SUBMIT,
            });
        }
        let active = self
            .inner
            .st
            .tasks
            .values()
            .filter(|r| !r.state.is_terminal())
            .count();
        if active + specs.len() > MAX_ACTIVE_TASKS {
            return Err(TaskError::Capacity {
                limit: MAX_ACTIVE_TASKS,
            });
        }
        let mut batch_ids: BTreeSet<&TaskId> = BTreeSet::new();
        for spec in &specs {
            validate_spec(spec, self.now)?;
            if self.inner.st.tasks.contains_key(&spec.id) || !batch_ids.insert(&spec.id) {
                return Err(TaskError::DuplicateId(spec.id.clone()));
            }
        }
        for spec in &specs {
            let known =
                |id: &TaskId| batch_ids.contains(id) || self.inner.st.tasks.contains_key(id);
            if let Some(dep) = spec.deps.iter().find(|d| !known(&d.task)) {
                return Err(TaskError::UnknownDependency {
                    task: spec.id.clone(),
                    dependency: dep.task.clone(),
                });
            }
            if let Some(parent) = spec.parent.as_ref().filter(|p| !known(p)) {
                return Err(TaskError::UnknownTask(parent.clone()));
            }
        }
        if let Some(path) = find_cycle(&specs) {
            return Err(TaskError::Cycle { path });
        }
        let ids: Vec<TaskId> = specs.iter().map(|s| s.id.clone()).collect();
        for spec in specs {
            self.insert(spec);
        }
        Ok(ids)
    }

    fn insert(&mut self, spec: TaskSpec) {
        let st = &mut self.inner.st;
        st.seq += 1;
        let deadline = effective_deadline(&spec, self.now);
        let payload = json!({
            "title": short_title(&spec.title),
            "class": spec.class,
            "origin": spec.origin,
            "assignee": spec.assignee,
            "parent": spec.parent,
            "deps": spec.deps.iter().map(|d| &d.task).collect::<Vec<_>>(),
            "resources": spec.resources,
            "tainted": spec.is_tainted(),
            "deadline_ms": deadline,
        });
        let id = spec.id.clone();
        if let Some(parent) = spec.parent.as_ref().and_then(|p| st.tasks.get_mut(p)) {
            parent.children.push(id.clone());
        }
        st.tasks
            .insert(id.clone(), TaskRec::new(spec, st.seq, self.now, deadline));
        self.event(EVENT_SUBMITTED, &id, payload);
        self.out.dirty = true;
    }

    /// Stan zależności zadania.
    pub(crate) fn deps_status(&self, rec: &TaskRec) -> DepsStatus {
        let mut waiting = false;
        for dep in &rec.spec.deps {
            match self.inner.st.tasks.get(&dep.task).map(|d| &d.state) {
                Some(TaskState::Done { termination }) => {
                    if !condition_met(&dep.condition, termination) {
                        return DepsStatus::Impossible(dep.task.clone(), dep.condition.clone());
                    }
                }
                Some(_) => waiting = true,
                None => {
                    return DepsStatus::Impossible(dep.task.clone(), dep.condition.clone());
                }
            }
        }
        if waiting {
            DepsStatus::Waiting
        } else {
            DepsStatus::Met
        }
    }

    /// `Pending` → `Ready` (zależności spełnione) albo `Skipped` (warunek niemożliwy).
    pub(crate) fn resolve_dependencies(&mut self) -> bool {
        let pending: Vec<TaskId> = self
            .inner
            .st
            .tasks
            .iter()
            .filter(|(_, r)| matches!(r.state, TaskState::Pending))
            .map(|(id, _)| id.clone())
            .collect();
        let mut progressed = false;
        for id in pending {
            let Some(rec) = self.inner.st.tasks.get(&id) else {
                continue;
            };
            match self.deps_status(rec) {
                DepsStatus::Waiting => {}
                DepsStatus::Met => {
                    if let Some(rec) = self.inner.st.tasks.get_mut(&id) {
                        rec.state = TaskState::Ready;
                        rec.blocked = None;
                        self.out.dirty = true;
                        progressed = true;
                    }
                }
                DepsStatus::Impossible(dependency, condition) => {
                    self.finalize(
                        &id,
                        Termination::Skipped {
                            dependency,
                            condition,
                        },
                    );
                    progressed = true;
                }
            }
        }
        progressed
    }

    /// Korzeń i wszyscy potomkowie w drzewie delegacji (BFS, kolejność deterministyczna).
    pub(crate) fn subtree(&self, root: &TaskId) -> Vec<TaskId> {
        let mut seen: BTreeSet<TaskId> = BTreeSet::new();
        let mut order = Vec::new();
        let mut queue = VecDeque::from([root.clone()]);
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(rec) = self.inner.st.tasks.get(&id) {
                queue.extend(rec.children.iter().cloned());
                order.push(id);
            }
        }
        order
    }

    /// Ustawia powód blokady (zdarzenie tylko przy zmianie).
    pub(crate) fn set_blocked(&mut self, id: &TaskId, reason: Option<BlockReason>) {
        let Some(rec) = self.inner.st.tasks.get_mut(id) else {
            return;
        };
        if rec.blocked == reason {
            return;
        }
        rec.blocked = reason.clone();
        self.out.dirty = true;
        if let Some(reason) = reason {
            self.event(
                crate::events::EVENT_BLOCKED,
                id,
                json!({ "reason": reason }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{Assignee, Dependency, TaskClass, TaskOrigin};

    fn spec(id: &str, deps: &[&str]) -> TaskSpec {
        TaskSpec::new(
            id,
            id,
            Assignee::AnyAgent,
            TaskClass::Agent,
            TaskOrigin::User,
        )
        .after(deps.iter().map(|d| Dependency::on(*d)))
    }

    #[test]
    fn cycles_are_found() {
        assert!(find_cycle(&[spec("a", &[]), spec("b", &["a"]), spec("c", &["a", "b"])]).is_none());
        let cycle = find_cycle(&[spec("a", &["c"]), spec("b", &["a"]), spec("c", &["b"])]);
        let cycle = cycle.unwrap_or_default();
        assert_eq!(cycle.first(), cycle.last());
        assert_eq!(cycle.len(), 4);
        let mut child = spec("x", &[]);
        child.parent = Some("y".into());
        let mut parent = spec("y", &[]);
        parent.parent = Some("x".into());
        assert!(find_cycle(&[child, parent]).is_some());
    }

    #[test]
    fn conditions() {
        use crate::state::{CancelCause, TaskOutput};
        let ok = Termination::Succeeded {
            output: TaskOutput::text("x").with("wynik", json!(3)),
        };
        let failed = Termination::Failed {
            error: "e".into(),
            attempts: 1,
        };
        let cancelled = Termination::Cancelled {
            cause: CancelCause::KillSwitch,
        };
        assert!(condition_met(&DepCondition::Succeeded, &ok));
        assert!(!condition_met(&DepCondition::Succeeded, &failed));
        assert!(condition_met(&DepCondition::Failed, &failed));
        assert!(!condition_met(&DepCondition::Failed, &cancelled));
        assert!(condition_met(&DepCondition::Finished, &cancelled));
        let eq = |v| DepCondition::OutputEquals {
            key: "wynik".into(),
            value: v,
        };
        assert!(condition_met(&eq(json!(3)), &ok));
        assert!(!condition_met(&eq(json!(4)), &ok));
        assert!(!condition_met(&eq(json!(3)), &failed));
    }
}
