//! Specyfikacja narzędzia `delegate_task`, opis obsady w prompcie Dyrygentki oraz
//! awaryjne wykrywanie tekstowego odesłania do innej agentki (np. gdy w starej sesji model
//! skopiuje z historii „Napisz „Delta, …”” zamiast wywołać `delegate_task`).

use agent_runtime_contract::{DELEGATE_GROUP, DELEGATE_TOOL};
use app_api::dto::AutonomyLevel;
use personas_contract::PersonaId;
use providers_contract::{ContentBlock, Message, ToolSpec, ToolUse};
use sessions_contract::SessionId;

use crate::engine::ChatEngine;

/// Kiedy Broker pyta właściciela o zgodę na danym poziomie — z tabeli reguł Brokera.
fn autonomy_note(level: AutonomyLevel) -> String {
    use risk_classifier_contract::{AutonomyLevel as Level, RULES, RuleId};
    let at = match level {
        AutonomyLevel::L0 => {
            return "L0 — agentka tylko czyta i podpowiada, nic nie zmienia".into();
        }
        AutonomyLevel::L1 => Level::L1,
        AutonomyLevel::L2 => Level::L2,
        AutonomyLevel::L3 => Level::L3,
        AutonomyLevel::L4 => Level::L4,
    };
    let asks: Vec<&str> = RULES
        .iter()
        .filter(|r| r.id != RuleId::KernelBlock && r.applies_at(at))
        .map(|r| r.description_pl)
        .collect();
    format!(
        "{at:?} — Broker pyta właściciela o zgodę, gdy: {}; twarde blokady Jądra obowiązują zawsze",
        asks.join("; ")
    )
}

/// Opis obsady sesji dla promptu Dyrygentki: kto jakie role pełni.
pub(crate) fn crew_roles_summary(core: &ChatEngine, session: &SessionId, agent: &str) -> String {
    let cast = core.inner.personas.cast(session);
    let personas = core.inner.personas.personas();
    let roles = core.inner.personas.roles();
    let me = PersonaId::new(agent);
    cast.personas()
        .into_iter()
        .filter(|p| *p != me)
        .filter_map(|pid| {
            let p = personas.iter().find(|x| x.id == pid)?;
            let rlist: Vec<String> = cast
                .roles_of(&pid)
                .into_iter()
                .filter_map(|rid| {
                    let r = roles.iter().find(|x| x.id == rid)?;
                    Some(format!("`{}` ({})", r.id, r.name))
                })
                .collect();
            (!rlist.is_empty()).then(|| format!("{} (`{}`): {}", p.name, p.id, rlist.join(", ")))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Informacja w prompcie: kto w obsadzie działa na komputerze, jak rozdzielać zadania i kiedy
/// Broker pyta o zgodę.
pub(crate) fn crew_note(
    core: &ChatEngine,
    session: &SessionId,
    agent: &str,
    crew: &[String],
    level: Option<AutonomyLevel>,
) -> String {
    let names = crew.join(", ");
    let roles_summary = crew_roles_summary(core, session, agent);
    let mut note = format!(
        "W tej rozmowie na komputerze użytkownika działa: {names} — pliki, programy, polecenia, \
         system, mysz i klawiatura, przez Brokera. Obsada sesji: {roles_summary}. Gdy prośba \
         wymaga wykonania planu, pracy w repozytorium, zmian w kodzie lub plikach, uruchomienia \
         testów, analizy albo zredagowania dokumentu — NIGDY nie odsyłaj użytkownika, żeby sam \
         napisał do innej agentki ani zmieniał adresatkę (nawet jeśli we wcześniejszych turach \
         rozmowy tak odpowiedziałaś). Sama rozdziel zadania i od razu wywołaj narzędzie \
         `delegate_task` (`role`, `persona`, `goal`), zlecając kolejne etapy odpowiednim agentkom \
         z obsady (agentki pracują nad swoimi zadaniami i zmieniają się zależnie od potrzeb), \
         a na końcu podsumuj wynik. Możesz również wskazać samą siebie w `persona`, gdy \
         potrzebujesz wykonać krok osobiście: dostęp do narzędzi określa profil tej agentki \
         w Ustawieniach. Do edycji pojedynczego pliku używaj narzędzi plikowych zamiast \
         powłoki kopiującej całe repozytorium przed każdym poleceniem. Błąd rozmiaru \
         snapshotu nie oznacza odmowy uprawnień: zmień metodę na edycję pliku lub zawęź cwd."
    );
    note.push_str(crate::conductor_pipeline::pipeline_note());
    if let Some(level) = level {
        note.push_str(&format!(
            " Poziom autonomii tej rozmowy: {}. Mówiąc o zgodach, trzymaj się tego opisu: nie \
             zapowiadaj zgody na każdą akcję, ale też nie obiecuj, że jej nie będzie — o każdej \
             akcji decyduje Broker.",
            autonomy_note(level)
        ));
    }
    note
}

/// Definicja narzędzia `delegate_task` dla Dyrygentki, gdy sesja ma katalog i aktywną obsadę.
pub(crate) fn delegate_tool_spec(
    core: &ChatEngine,
    session: &SessionId,
    agent: &str,
) -> Option<ToolSpec> {
    let _stack = core.inner.agents.as_ref()?;
    core.session_workdir(session)?;
    let me = PersonaId::new(agent);
    let my_roles = core.cast_roles(session, &me);
    if !my_roles
        .iter()
        .any(|r| r.tools.iter().any(|g| g == DELEGATE_GROUP) && !r.read_only)
    {
        return None;
    }
    let summary = crew_roles_summary(core, session, agent);
    if summary.is_empty() {
        return None;
    }
    Some(ToolSpec {
        name: DELEGATE_TOOL.to_owned(),
        description: format!(
            "Zleca podzadanie odpowiedniej agentce i roli z obsady w tej samej rozmowie. \
             Wywołuj to narzędzie, gdy właściciel prosi o wykonanie planu, zmian w kodzie lub \
             plikach, uruchomienie poleceń/testów, analizę albo zredagowanie dokumentu — możesz \
             wywoływać je wielokrotnie (kolejno dla różnych agentek zależnie od potrzeb). \
             Dostępna obsada: {summary}."
        ),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "role": { "type": "string", "description": "Rola wykonawczyni (np. coder, operator, researcher, critic, thinker, writer, keeper)." },
                "persona": { "type": "string", "description": "Identyfikator lub imię agentki (np. delta, gama, beta)." },
                "model": { "type": "string", "description": "Opcjonalnie model podzadania w zapisie dostawca:model (pusty = dobór Routera)." },
                "goal": { "type": "string", "description": "Konkretne zadanie do wykonania wraz z kryterium ukończenia." }
            },
            "required": ["role", "goal"]
        }),
        strict: false,
    })
}

/// Czy wiadomość modelu zawiera wywołanie `delegate_task`.
pub(crate) fn has_delegate_calls(message: &Message) -> bool {
    message
        .content
        .iter()
        .any(|b| matches!(b, ContentBlock::ToolUse(u) if crate::conductor_pipeline::is_conductor_tool(&u.name)))
}

/// Gdy w starej sesji model mimo narzędzia `delegate_task` powtórzy tekstowe odesłanie z historii
/// („Napisz „Delta, …”” / „wybierz Deltę w polu adresatki”), automatycznie zamienia je na
/// wywołanie `delegate_task` z poleceniem użytkownika.
pub(crate) fn ensure_delegate_call_if_handoff(message: &mut Message, user_goal: &str) -> bool {
    if has_delegate_calls(message) {
        return true;
    }
    let goal = user_goal.trim();
    if goal.is_empty() || goal.ends_with('?') {
        return false;
    }
    let text = message.visible_text().to_lowercase();
    let handoff = text.contains("w polu adresatki")
        || text.contains("napisz „delta")
        || text.contains("napisz \"delta")
        || text.contains("wybierz deltę");
    if !handoff {
        return false;
    }
    message.content.push(ContentBlock::ToolUse(ToolUse {
        id: "auto-delegate-1".into(),
        name: DELEGATE_TOOL.to_owned(),
        input: serde_json::json!({
            "role": "coder",
            "persona": "delta",
            "goal": goal
        }),
    }));
    true
}

/// Czy narzędzie jest obsługiwane przez orkiestrację Dyrygentki.
pub(crate) fn is_conductor_tool(name: &str) -> bool {
    name == crate::conductor_pipeline::PIPELINE_TOOL
        || name == agent_runtime_contract::DELEGATE_TOOL
}

/// Opcjonalny model podzadania `delegate_task` (`dostawca:model`); błędny zapis = dobór Routera.
pub(crate) fn model_of(input: &serde_json::Value) -> Option<String> {
    input
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| router_contract::Candidate::parse(s).is_some())
        .map(str::to_owned)
}
