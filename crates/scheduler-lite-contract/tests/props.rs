//! ACC-F2-scheduler-lite-01 / F5-03: 1000 losowych scenariuszy (≥ 4 posiadaczki, 3 zasoby):
//! wyłączność w każdej chwili, 0 zakleszczeń, każde żądanie rozstrzygnięte dokładnie raz,
//! timeouty respektowane, na końcu każda dzierżawa zwolniona.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use personas_contract::PersonaId;
use proptest::prelude::*;
use scheduler_lite_contract::{
    Effect, Holder, LeaseId, LeaseRequest, LockTable, PreemptReason, Priority, RequestId, Resource,
};

const HOLDERS: [&str; 5] = ["alfa", "beta", "gama", "delta", "user"];
const PRIORITIES: [Priority; 5] = [
    Priority::Background,
    Priority::Narration,
    Priority::Normal,
    Priority::Interactive,
    Priority::UserSpeech,
];

fn holder(i: usize) -> Holder {
    match HOLDERS[i % HOLDERS.len()] {
        "user" => Holder::User,
        name => Holder::Persona(PersonaId::new(name)),
    }
}

fn resource(i: usize) -> Resource {
    [
        Resource::Speaker,
        Resource::ScreenInput,
        Resource::file("C:/dane.txt"),
    ][i % 3]
        .clone()
}

#[derive(Debug, Clone)]
enum Op {
    Request {
        h: usize,
        r: usize,
        p: usize,
        wait: u64,
    },
    Release(usize),
    Advance(u64),
    Preempt(usize),
    Handoff {
        lease: usize,
        to: usize,
    },
    AtomicPoint,
    KillAll,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => (0usize..5, 0usize..3, 0usize..5, 0u64..400).prop_map(|(h, r, p, wait)| Op::Request { h, r, p, wait }),
        4 => (0usize..8).prop_map(Op::Release),
        3 => (0u64..250).prop_map(Op::Advance),
        1 => (0usize..3).prop_map(Op::Preempt),
        1 => (0usize..8, 0usize..5).prop_map(|(lease, to)| Op::Handoff { lease, to }),
        2 => Just(Op::AtomicPoint),
        1 => Just(Op::KillAll),
    ]
}

#[derive(Default)]
struct Model {
    kinds: BTreeMap<&'static str, usize>,
    active: BTreeMap<LeaseId, Resource>,
    preempt_requested: BTreeSet<LeaseId>,
    outcomes: BTreeMap<RequestId, usize>,
    requested: BTreeSet<RequestId>,
}

impl Model {
    fn absorb(&mut self, effects: &[Effect]) {
        for effect in effects {
            let kind = match effect {
                Effect::Granted(_) => "granted",
                Effect::Released(_) => "released",
                Effect::HandedOff { .. } => "handoff",
                Effect::Revoked(..) => "revoked",
                Effect::PreemptRequested { .. } => "preempt",
                Effect::TimedOut { .. } => "timeout",
                Effect::Deadlock { .. } => "deadlock",
                Effect::Cancelled { .. } => "cancelled",
                Effect::Queued { .. } => "queued",
            };
            *self.kinds.entry(kind).or_default() += 1;
            let resolved = match effect {
                Effect::Granted(l) => {
                    self.active.insert(l.id, l.resource.clone());
                    Some(l.request)
                }
                Effect::Released(l)
                | Effect::HandedOff { lease: l, .. }
                | Effect::Revoked(l, _) => {
                    assert!(
                        self.active.remove(&l.id).is_some(),
                        "zwolniono nieaktywną dzierżawę"
                    );
                    self.preempt_requested.remove(&l.id);
                    None
                }
                Effect::PreemptRequested { lease, .. } => {
                    self.preempt_requested.insert(lease.id);
                    None
                }
                Effect::TimedOut { request, .. }
                | Effect::Deadlock { request, .. }
                | Effect::Cancelled { request, .. } => Some(request.id),
                Effect::Queued { .. } => None,
            };
            if let Some(id) = resolved {
                *self.outcomes.entry(id).or_default() += 1;
                assert_eq!(
                    self.outcomes[&id], 1,
                    "żądanie {id:?} rozstrzygnięte dwa razy"
                );
            }
        }
    }
}

fn check_invariants(table: &LockTable, model: &Model, now: u64) {
    // Wyłączność: najwyżej jedna dzierżawa na zasób; model = tablica.
    let leases = table.leases();
    let resources: BTreeSet<&Resource> = leases.iter().map(|l| &l.resource).collect();
    assert_eq!(
        resources.len(),
        leases.len(),
        "dwie posiadaczki jednego zasobu"
    );
    let ids: BTreeSet<LeaseId> = leases.iter().map(|l| l.id).collect();
    assert_eq!(ids, model.active.keys().copied().collect());
    // 0 zakleszczeń.
    assert_eq!(table.find_cycle(), None);
    for r in 0..3 {
        let res = resource(r);
        let queue = table.queue(&res);
        // Kolejka: priorytet malejąco, potem FIFO.
        for pair in queue.windows(2) {
            assert!(
                (pair[0].priority, std::cmp::Reverse(pair[0].id))
                    > (pair[1].priority, std::cmp::Reverse(pair[1].id))
            );
        }
        // Timeouty respektowane (po `tick`): nikt nie czeka po terminie.
        assert!(
            queue.iter().all(|q| q.deadline_ms > now),
            "czeka po terminie"
        );
    }
}

fn run(ops: Vec<Op>) -> BTreeMap<&'static str, usize> {
    let mut table = LockTable::new();
    let mut model = Model::default();
    let mut now = 0u64;
    for op in ops {
        let effects = match op {
            Op::Request { h, r, p, wait } => {
                let req = LeaseRequest::new(
                    resource(r),
                    holder(h),
                    PRIORITIES[p],
                    Duration::from_millis(wait),
                );
                match table.request(req, now) {
                    Ok((id, effects)) => {
                        assert!(model.requested.insert(id), "id żądania nie jest unikatowe");
                        effects
                    }
                    Err(_) => Vec::new(),
                }
            }
            Op::Release(i) => match model
                .active
                .keys()
                .nth(i % model.active.len().max(1))
                .copied()
            {
                Some(id) => table.release(id, now),
                None => Vec::new(),
            },
            Op::Advance(dt) => {
                now += dt;
                table.tick(now)
            }
            Op::Preempt(r) => table
                .preempt(&resource(r), Holder::User, PreemptReason::UserSpeaks, now)
                .unwrap_or_default(),
            Op::Handoff { lease, to } => match model
                .active
                .keys()
                .nth(lease % model.active.len().max(1))
                .copied()
            {
                Some(id) => table.handoff(id, holder(to), now).unwrap_or_default(),
                None => Vec::new(),
            },
            Op::AtomicPoint => {
                // Posiadaczki poproszone o zwolnienie robią to w punkcie atomowym.
                let asked: Vec<LeaseId> = model.preempt_requested.iter().copied().collect();
                let mut all = Vec::new();
                for id in asked {
                    let effects = table.release(id, now);
                    model.absorb(&effects);
                    all.extend(effects);
                }
                check_invariants(&table, &model, now);
                continue;
            }
            Op::KillAll => table.kill_all(),
        };
        model.absorb(&effects);
        check_invariants(&table, &model, now);
    }
    // Koniec: posiadaczki zwalniają wszystko, czas płynie → wszystko rozstrzygnięte i zwolnione.
    for _ in 0..64 {
        let Some(id) = model.active.keys().next().copied() else {
            break;
        };
        let effects = table.release(id, now);
        model.absorb(&effects);
        check_invariants(&table, &model, now);
    }
    now += 1_000_000;
    let effects = table.tick(now);
    model.absorb(&effects);
    while let Some(id) = model.active.keys().next().copied() {
        let effects = table.release(id, now);
        model.absorb(&effects);
    }
    assert!(table.is_idle(), "zostały dzierżawy/żądania/rezerwacje");
    for id in &model.requested {
        assert_eq!(
            model.outcomes.get(id),
            Some(&1),
            "żądanie {id:?} nierozstrzygnięte"
        );
    }
    model.kinds
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]
    #[test]
    fn random_scenarios_keep_invariants(ops in prop::collection::vec(op(), 1..120)) {
        run(ops);
    }
}

/// Generator scenariuszy rzeczywiście dochodzi do wszystkich rodzajów decyzji (pokrycie testu).
#[test]
fn scenarios_cover_all_decisions() {
    use proptest::strategy::ValueTree;
    let mut runner = proptest::test_runner::TestRunner::deterministic();
    let strategy = prop::collection::vec(op(), 60..120);
    let mut total: BTreeMap<&'static str, usize> = BTreeMap::new();
    for _ in 0..300 {
        let ops = strategy.new_tree(&mut runner).unwrap().current();
        for (kind, n) in run(ops) {
            *total.entry(kind).or_default() += n;
        }
    }
    for kind in [
        "granted",
        "released",
        "handoff",
        "revoked",
        "preempt",
        "timeout",
        "deadlock",
        "cancelled",
        "queued",
    ] {
        assert!(
            total.get(kind).copied().unwrap_or(0) > 0,
            "brak decyzji {kind}: {total:?}"
        );
    }
    eprintln!("pokrycie decyzji w 300 scenariuszach: {total:?}");
}

#[test]
fn deterministic_same_input_same_effects() {
    let script = |table: &mut LockTable| -> Vec<String> {
        let mut log = Vec::new();
        for (i, (h, r)) in [(0, 0), (1, 0), (2, 1), (3, 0), (1, 1), (0, 1)]
            .into_iter()
            .enumerate()
        {
            let req = LeaseRequest::new(
                resource(r),
                holder(h),
                PRIORITIES[i % 5],
                Duration::from_millis(100),
            );
            log.push(format!("{:?}", table.request(req, i as u64)));
        }
        log.push(format!("{:?}", table.tick(500)));
        log
    };
    assert_eq!(script(&mut LockTable::new()), script(&mut LockTable::new()));
}
