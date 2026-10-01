//! Fixture'y testów kontraktowych: dokumenty wszystkich kategorii, sesje z gałęziami, prywatna
//! sesja, sekrety (także „wklejone” w treść — do testu szpiegowskiego).

use accounts_hub_contract::{SecretName, SecretString};
use sessions_contract::{
    HeardPrefix, NewSession, NewTurn, PrivacyTag, SessionId, SessionPatch, TurnId,
};

use super::{Harness, ok};
use crate::scope::Category;

/// Sekret pasujący do wzorców `RegexRedactor` (klucz Anthropic).
pub const SECRET_PATTERN: &str = "sk-ant-api03-SEKRETNYKLUCZ1234567890";
/// Sekret bez rozpoznawalnego formatu (wykrywany tylko dokładnie, z `SecretStore`).
pub const SECRET_PLAIN: &str = "zwykle-haslo-bez-wzorca-42";

/// Identyfikatory utworzone przez [`seed`].
#[derive(Debug, Clone)]
pub struct Seeded {
    /// Sesja z gałęziami, prefiksem, ukrytą turą i szkicem.
    pub s1: SessionId,
    /// Aktywny liść `s1`.
    pub s1_leaf: TurnId,
    /// Druga sesja (zarchiwizowana).
    pub s2: SessionId,
    /// Sesja prywatna.
    pub private: SessionId,
}

/// Konfiguracja wspólna fixture'a.
pub const SHARED_TOML: &str = "[voice]\nengine = \"piper\"\n\n[ui]\ntheme = \"dark\"\n";

fn put_doc(h: &dyn Harness, category: Category, name: &str, text: &str) {
    ok(h.store(category).write(name, text.as_bytes()));
}

/// Wypełnia porty; `secrets_in_content` — sekrety także w treści sesji, konfiguracji i pamięci.
pub fn seed(h: &dyn Harness, secrets_in_content: bool) -> Seeded {
    let s = h.sessions();
    let s1 = ok(s.create_session(NewSession {
        title: "Projekt Żółć".into(),
        tags: vec!["praca".into()],
        ..NewSession::default()
    }))
    .id;
    let u1 = ok(s.append_turn(&s1, None, NewTurn::user("Jak zacząć projekt?")));
    let mut a1 = NewTurn::assistant("alfa", "Najpierw plan, potem szkielet i testy.");
    a1.heard_prefix = Some(HeardPrefix {
        chars: 12,
        approximate: true,
    });
    let a1 = ok(s.append_turn(&s1, Some(u1.id), a1));
    let text = if secrets_in_content {
        format!("Mój klucz to {SECRET_PLAIN}, nie mów nikomu")
    } else {
        "Dziękuję, a co z dokumentacją?".to_owned()
    };
    let u2 = ok(s.append_turn(&s1, Some(a1.id), NewTurn::user(text)));
    ok(s.fork_from(
        &s1,
        a1.id,
        NewTurn::assistant("beta", "Wariant odpowiedzi."),
    ));
    let leaf = ok(s.append_turn(
        &s1,
        Some(u2.id),
        NewTurn::assistant("alfa", "Dokumentacja na końcu."),
    ));
    ok(s.set_hidden(&s1, u2.id, true));
    ok(s.set_active_leaf(&s1, leaf.id));
    ok(s.save_draft(&s1, "szkic wiadomości"));
    ok(s.update_session(
        &s1,
        SessionPatch {
            pinned: Some(true),
            ..SessionPatch::default()
        },
    ));

    let s2 = ok(s.create_session(NewSession {
        title: "Druga".into(),
        ..NewSession::default()
    }))
    .id;
    let q = ok(s.append_turn(&s2, None, NewTurn::user("Pytanie drugie")));
    let answer = if secrets_in_content {
        format!("Użyj klucza {SECRET_PATTERN} w nagłówku")
    } else {
        "Odpowiedź druga.".to_owned()
    };
    ok(s.append_turn(&s2, Some(q.id), NewTurn::assistant("alfa", answer)));
    ok(s.update_session(
        &s2,
        SessionPatch {
            archived: Some(true),
            ..SessionPatch::default()
        },
    ));

    let private = ok(s.create_session(NewSession {
        title: "Prywatna".into(),
        privacy: PrivacyTag::Private,
        ..NewSession::default()
    }))
    .id;
    ok(s.append_turn(&private, None, NewTurn::user("Sprawa prywatna")));

    let shared = if secrets_in_content {
        format!("{SHARED_TOML}\n[providers.x]\napi_key = \"{SECRET_PLAIN}\"\n")
    } else {
        SHARED_TOML.to_owned()
    };
    put_doc(h, Category::ConfigCommon, "shared.toml", &shared);
    put_doc(
        h,
        Category::ConfigMachine,
        &format!("{}.toml", h.machine_id()),
        "[audio]\ninput = \"mikrofon\"\n",
    );
    put_doc(
        h,
        Category::Personas,
        "personas.json",
        "{\"version\":1,\"personas\":[{\"id\":\"gama\",\"name\":\"Gama\"}]}",
    );
    put_doc(
        h,
        Category::Casts,
        "standard.json",
        "{\"template\":\"standard\"}",
    );
    put_doc(h, Category::Rules, "marszalek.json", "{\"rules\":[]}");
    let memory = if secrets_in_content {
        format!(
            "{{\"id\":1,\"text\":\"lubi kawę\"}}\n{{\"id\":2,\"text\":\"token: {SECRET_PLAIN}\"}}\n"
        )
    } else {
        "{\"id\":1,\"text\":\"lubi kawę\"}\n{\"id\":2,\"text\":\"pracuje rano\"}\n".to_owned()
    };
    put_doc(h, Category::Memory, "global.ndjson", &memory);
    put_doc(
        h,
        Category::Artifacts,
        &format!("{s1}/raport.md"),
        "# Raport\n",
    );
    put_doc(h, Category::Logs, "alfa.ndjson", "{\"msg\":\"start\"}\n");

    for (name, value) in [
        ("accounts/acc-1", SECRET_PATTERN),
        ("accounts/acc-2", SECRET_PLAIN),
    ] {
        let name = ok(SecretName::new(name));
        ok(h.secrets().put(&name, &SecretString::from(value)));
    }
    Seeded {
        s1,
        s1_leaf: leaf.id,
        s2,
        private,
    }
}
