//! Dobór według trudności w `RouterCore`: szczebel każdego kandydata (ręczny z ustawień, lekki
//! dostawca lokalny, rozpoznanie po nazwie), trudność zadania (wskazówka z żądania, sprawdzanie,
//! ocena treści) z dolną granicą klasy, podniesienie po odrzuceniu odpowiedzi i ułożenie
//! kolejności prób (`router_contract::order_by_rung`).

use providers_contract::{ChatRequest, estimate_request, estimate_text};
use router_contract::{
    Candidate, Constraints, DIFFICULTY_CLASSES, DifficultyNote, DifficultyPolicy, Ranked,
    RouteKind, Rung, TaskClass, class_floor, class_from_policy, order_by_rung, rung_by_name,
};

use crate::routing::RouterCore;

/// Nazwa klasy w powodzie podniesienia trudności do granicy klasy.
fn class_label(class: TaskClass) -> &'static str {
    match class {
        TaskClass::Code => "zadanie kodu",
        TaskClass::Planning => "planowanie",
        TaskClass::GuiVision => "sterowanie ekranem",
        _ => "rozmowa",
    }
}

impl RouterCore {
    /// Ustawienia doboru według trudności.
    pub fn difficulty_policy(&self) -> DifficultyPolicy {
        self.difficulty
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Zmienia ustawienia doboru (działa od następnej decyzji).
    pub fn set_difficulty_policy(&self, policy: DifficultyPolicy) {
        *self.difficulty.write().unwrap_or_else(|p| p.into_inner()) = policy;
    }

    /// Szczebel kandydata: ręczny z ustawień, lekki dostawca lokalny, potem rozpoznanie po nazwie.
    pub fn rung_of(&self, candidate: &Candidate) -> Rung {
        if let Some(rung) = self
            .difficulty
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .rungs
            .get(&candidate.qualified())
        {
            return *rung;
        }
        if self.light_id().as_ref() == Some(&candidate.provider) {
            return Rung::Light;
        }
        rung_by_name(&candidate.model)
    }

    /// Szczebel ustalony bez ręcznych ustawień (do podpowiedzi w interfejsie).
    pub fn automatic_rung(&self, candidate: &Candidate) -> Rung {
        if self.light_id().as_ref() == Some(&candidate.provider) {
            Rung::Light
        } else {
            rung_by_name(&candidate.model)
        }
    }

    /// Klasa zadania: klasa roli ze wskazówki żądania, inaczej klasa wywołującego.
    pub fn class_for(&self, own: TaskClass, request: &ChatRequest) -> TaskClass {
        request
            .meta
            .route
            .class
            .as_deref()
            .and_then(class_from_policy)
            .unwrap_or(own)
    }

    /// Trudność zadania i jej powód; `None` — klasa bez doboru, dobór wyłączony albo brak treści.
    pub fn judge(
        &self,
        class: TaskClass,
        constraints: &Constraints,
        request: Option<&ChatRequest>,
    ) -> Option<DifficultyNote> {
        let policy = self.difficulty_policy();
        if !policy.enabled || !DIFFICULTY_CLASSES.contains(&class) {
            return None;
        }
        let hint = &constraints.hint;
        let floor = class_floor(class);
        let (difficulty, score, mut reasons) = if hint.review {
            (policy.review, None, "sprawdzenie odpowiedzi".to_owned())
        } else if let Some(d) = hint.difficulty {
            (d, None, "ustalona dla całego zadania".to_owned())
        } else if let Some(task) = &hint.task {
            let estimate = estimate_text(task, false, false);
            let level = estimate.level(policy.strategy);
            (level, Some(estimate.score), estimate.reasons())
        } else {
            let estimate = estimate_request(request?);
            let level = estimate.level(policy.strategy);
            (level, Some(estimate.score), estimate.reasons())
        };
        let difficulty = if !hint.review && difficulty < floor {
            reasons.push_str(&format!("; {}", class_label(class)));
            floor
        } else {
            difficulty
        };
        let escalated_from = hint
            .escalate_from
            .as_deref()
            .and_then(Candidate::parse)
            .filter(|_| policy.escalate);
        let mut start = Rung::for_difficulty(difficulty);
        if let Some(from) = &escalated_from {
            start = start.max(self.rung_of(from).up());
        }
        let kept_queue = !policy.reorder_queues && self.manual_queue(class);
        Some(DifficultyNote {
            difficulty,
            score,
            reasons,
            start,
            escalated_from,
            kept_queue,
        })
    }

    /// Kolejność prób według szczebli od szczebla startowego notatki.
    pub(crate) fn arrange(
        &self,
        class: TaskClass,
        list: Vec<Candidate>,
        note: &DifficultyNote,
        review: bool,
    ) -> Vec<Candidate> {
        let local_first = self.difficulty_policy().local_first;
        let ranked = list
            .into_iter()
            .map(|candidate| Ranked {
                rung: self.rung_of(&candidate),
                local: self
                    .registered(&candidate.provider)
                    .is_some_and(|r| r.kind == RouteKind::Local),
                candidate,
            })
            .collect();
        let ordered = order_by_rung(
            ranked,
            note.start,
            local_first,
            note.escalated_from.as_ref(),
        );
        crate::main_local::prefer(self, class, ordered, note, review)
    }
}
