//! Polecenie i obsada przebiegu zadania w tle. Zadanie z arkuszem faktów ([`crate::facts`]) biegnie
//! bez delegacji: jego forma ([`crate::facts::REPORT_FORM`]) zabrania przekazywania zadania innej
//! agentce, a próba 0.0.64 pokazała, że mały model lokalny w kopercie JSON (C2.3) wywołuje jedyne
//! podane narzędzie — delegację — w każdej turze mimo gotowego raportu, aż do `LoopDetected`.
//! Dotyczy każdego nieskażonego zadania z wyzwalacza albo harmonogramu, nie tylko gotowych
//! obowiązków.

use agent_runtime_contract::DELEGATE_GROUP;
use personas_contract::Role;

/// Zdanie dla przebiegu bez delegacji — prompt roli (Dyrygentka, Głos) mówi o delegowaniu, a
/// narzędzia do tego w tym przebiegu nie ma; bez tego zdania prompt i zestaw narzędzi sobie
/// przeczą.
pub const NO_DELEGATION: &str = "W tym zadaniu w tle delegowanie jest niedostępne: wykonaj je sama, bez przekazywania innej agentce.";

/// Role bez grupy `delegate` — przebieg nie dostaje `delegate_task`. Role zostają (prompt roli),
/// Krytyczka też (obsada w opcjach przebiegu się nie zmienia).
pub(crate) fn without_delegation(mut roles: Vec<Role>) -> Vec<Role> {
    for role in &mut roles {
        role.tools.retain(|group| group != DELEGATE_GROUP);
    }
    roles
}

/// Polecenie i role przebiegu: z arkuszem faktów — cel, arkusz i [`NO_DELEGATION`], role bez
/// delegacji; bez arkusza — sam cel i role bez zmian.
pub(crate) fn brief_and_roles(
    goal: &str,
    sheet: Option<String>,
    roles: &[Role],
) -> (String, Vec<Role>) {
    match sheet {
        Some(sheet) => (
            format!("{goal}\n\n{sheet}\n{NO_DELEGATION}"),
            without_delegation(roles.to_vec()),
        ),
        None => (goal.to_owned(), roles.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cast() -> Vec<Role> {
        personas_contract::builtin_roles()
            .into_iter()
            .filter(|r| ["conductor", "speaker", "writer"].contains(&r.id.as_str()))
            .collect()
    }

    fn delegates(roles: &[Role]) -> bool {
        roles
            .iter()
            .any(|r| r.tools.iter().any(|g| g == DELEGATE_GROUP))
    }

    #[test]
    fn report_tasks_lose_only_the_delegate_group() {
        let roles = cast();
        assert!(delegates(&roles));
        let stripped = without_delegation(roles.clone());
        assert_eq!(
            stripped.len(),
            roles.len(),
            "role zostają (prompt i Krytyczka)"
        );
        assert!(!delegates(&stripped));
        let tools = |rs: &[Role]| {
            rs.iter()
                .find(|r| r.id.as_str() == "writer")
                .map(|r| r.tools.clone())
        };
        assert_eq!(
            tools(&stripped),
            tools(&roles),
            "inne grupy narzędzi bez zmian"
        );
    }

    #[test]
    fn sheet_strips_delegation_and_lands_in_brief() {
        let roles = cast();
        let (brief, run_roles) =
            brief_and_roles("Przegląd dnia", Some("<dane-programu>".into()), &roles);
        assert!(brief.starts_with("Przegląd dnia"));
        assert!(brief.contains("<dane-programu>") && brief.contains(NO_DELEGATION));
        assert_eq!(run_roles.len(), roles.len());
        assert!(!delegates(&run_roles));
    }

    #[test]
    fn without_sheet_roles_and_goal_stay() {
        let roles = cast();
        let (brief, run_roles) = brief_and_roles("Posortuj pliki", None, &roles);
        assert_eq!(brief, "Posortuj pliki");
        assert_eq!(run_roles, roles);
        assert!(delegates(&run_roles));
    }
}
