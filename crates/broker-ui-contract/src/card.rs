//! Model prezentacji prośby o zatwierdzenie (karta Broker-UI, makieta 15): kto, co, zakres,
//! ryzyko, odwracalność, źródło polecenia, plan wielu akcji, opcje decyzji i czas do wygaśnięcia.
//! Cały tekst jest czystym tekstem po sanityzacji (bez znaków sterujących, bidi i zerowej
//! szerokości), obcięty do limitu — Broker-UI niczego nie renderuje jako HTML/markdown.

use risk_classifier_contract::{AutonomyLevel, CommandOrigin, Reversibility, RiskLevel};
use safety_broker_contract::{
    ApprovalId, ApprovalRequest, ApprovalSubject, AutonomyTarget, Capability, Holder, KernelPolicy,
};
use serde::{Deserialize, Serialize};

/// Najdłuższy „zawsze zezwalaj w tym zakresie” (24 h, PLAN §14.6).
pub const GRANT_MAX_MS: u64 = 24 * 60 * 60 * 1000;
/// Limit znaków jednej wartości na karcie.
pub const MAX_VALUE_CHARS: usize = 300;
/// Ile kroków planu pokazujemy wprost (reszta jako „… i jeszcze N”).
pub const MAX_PLAN_LINES: usize = 8;

/// Kto prosi (persona).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonaDisplay {
    /// Imię (np. „Delta”) albo „Jądro Alfy”.
    pub name: String,
}

/// Opcja decyzji na karcie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "option", rename_all = "snake_case")]
pub enum DecisionOption {
    /// „Tylko teraz”.
    AllowOnce,
    /// „Zawsze w tym zakresie” do terminu (≤ 24 h; nigdy nie eskaluje do L4).
    AllowInScope {
        /// Zakres = zdolność prośby (UI nie poszerza zakresu).
        scope: Capability,
        /// Do kiedy (ms).
        until_ms: u64,
    },
    /// „Odmów”.
    Deny,
}

/// Krok planu na karcie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanLine {
    /// Opis kroku (zsanityzowany).
    pub description: String,
    /// Zdolność kroku (tekst).
    pub capability: String,
    /// Ryzyko kroku.
    pub risk: RiskLevel,
}

/// Karta zatwierdzenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalCard {
    /// Prośba.
    pub id: ApprovalId,
    /// Kto prosi.
    pub persona: PersonaDisplay,
    /// Tytuł.
    pub title: String,
    /// Wiersze „etykieta: wartość”.
    pub details: Vec<(String, String)>,
    /// Ryzyko.
    pub risk: RiskLevel,
    /// Odwracalność.
    pub reversible: Reversibility,
    /// Polecenie głosowe (potwierdzenie wyłącznie tutaj, kliknięciem/klawiszem).
    pub voice_origin: bool,
    /// Sesja `tainted`.
    pub tainted: bool,
    /// Kroki planu (dla „planu do zatwierdzenia”).
    pub plan: Vec<PlanLine>,
    /// Opcje (kolejność przycisków; „Odmów” zawsze pierwsza i z fokusem).
    pub options: Vec<DecisionOption>,
    /// Tekst do odczytania na głos przez `voice-tts` (zlecenie, nie zatwierdzenie).
    pub read_aloud: Option<String>,
    /// Wymagane Windows Hello.
    pub hello_required: bool,
    /// Utworzono (ms).
    pub created_at_ms: u64,
    /// Wygasa (ms).
    pub expires_at_ms: u64,
}

fn invisible(c: char) -> bool {
    matches!(c,
        '\u{00AD}' | '\u{061C}' | '\u{180E}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}' | '\u{FFF9}'..='\u{FFFB}')
}

/// Sanityzacja tekstu z LLM/narzędzi: znaki sterujące i białe → spacja (zwinięte), znaki bidi
/// i zerowej szerokości usunięte (bez „Trojan Source” w etykietach), obcięcie z „…”.
pub fn sanitize(text: &str, max_chars: usize) -> String {
    let mut chars: Vec<char> = Vec::new();
    for c in text.chars().filter(|c| !invisible(*c)) {
        let c = if c.is_whitespace() || c.is_control() {
            ' '
        } else {
            c
        };
        if c == ' ' && chars.last().is_none_or(|l| *l == ' ') {
            continue;
        }
        chars.push(c);
    }
    while chars.last() == Some(&' ') {
        chars.pop();
    }
    if chars.len() > max_chars {
        chars.truncate(max_chars.saturating_sub(1));
        while chars.last() == Some(&' ') {
            chars.pop();
        }
        chars.push('…');
    }
    chars.into_iter().collect()
}

/// Czas do wygaśnięcia po polsku („4 min 30 s”).
pub fn time_left_pl(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..60 => format!("{s} s"),
        60..3600 if s.is_multiple_of(60) => format!("{} min", s / 60),
        60..3600 => format!("{} min {} s", s / 60, s % 60),
        _ => format!("{} h {} min", s / 3600, (s % 3600) / 60),
    }
}

/// Imię z identyfikatora agentki (`delta` → „Delta”); bez agentki — jądro.
pub fn persona_of(holder: &Holder) -> PersonaDisplay {
    let name = holder.agent.as_ref().map_or_else(
        || "Jądro Alfy".to_owned(),
        |a| {
            let clean = sanitize(a.as_str(), 40);
            let mut cs = clean.chars();
            cs.next()
                .map_or_else(String::new, |f| f.to_uppercase().chain(cs).collect())
        },
    );
    PersonaDisplay { name }
}

fn verb(cap: &Capability) -> &'static str {
    match cap {
        Capability::FsRead(_) => "odczyt plików",
        Capability::FsWrite(_) => "zmiana lub usunięcie plików",
        Capability::ShellExec(_) => "polecenie powłoki",
        Capability::GuiControl(_) => "sterowanie aplikacją",
        Capability::NetEgress(_) => "wysłanie danych do sieci",
        Capability::SecretsRead(_) => "odczyt sekretu",
        Capability::SystemAdmin(_) => "operacja administratora",
    }
}

fn origin_pl(o: &CommandOrigin) -> &'static str {
    match o {
        CommandOrigin::UserText => "Ty (tekst)",
        CommandOrigin::UserVoice { .. } => "Ty (głos) — potwierdź kliknięciem lub klawiszem",
        CommandOrigin::Agent => "agentka (z własnej inicjatywy)",
        CommandOrigin::UntrustedContent => "NIEZAUFANA TREŚĆ (strona, mail, plik, dźwięk)",
    }
}

fn reversible_pl(r: Reversibility) -> &'static str {
    match r {
        Reversibility::Yes => "tak (dziennik cofania)",
        Reversibility::Scoped => "w zakresie (migawka przed wykonaniem)",
        Reversibility::No => "NIE — nieodwracalne",
    }
}

fn level_pl(l: AutonomyLevel) -> String {
    format!("{l:?} „{}”", l.name_pl())
}

fn target_pl(t: &AutonomyTarget) -> String {
    let name = |a: &str| sanitize(a, 40);
    match t {
        AutonomyTarget::Global => "cała aplikacja".into(),
        AutonomyTarget::Session { session } => format!("sesja {}", name(session.as_str())),
        AutonomyTarget::Agent { agent } => format!("agentka {}", name(agent.as_str())),
        AutonomyTarget::SessionAgent { session, agent } => format!(
            "agentka {} w sesji {}",
            name(agent.as_str()),
            name(session.as_str())
        ),
    }
}

fn policy_pl(p: &KernelPolicy) -> String {
    format!(
        "TTL tokenu {} min (maks. {} min), egress: {} hostów, aplikacje: {}, Hello dla: {:?}",
        p.token_ttl_default_ms / 60_000,
        p.token_ttl_max_ms / 60_000,
        p.egress_allowlist.len(),
        p.allowed_apps.len(),
        p.hello_required_for
    )
}

/// Opcje karty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardOptions {
    /// Czas „zawsze w tym zakresie” (przycinany do [`GRANT_MAX_MS`]).
    pub grant_ms: u64,
}

impl Default for CardOptions {
    fn default() -> Self {
        Self {
            grant_ms: 8 * 60 * 60 * 1000,
        }
    }
}

impl ApprovalCard {
    /// Buduje kartę z prośby Brokera (czysta funkcja — te same dane dają tę samą kartę).
    pub fn from_request(req: &ApprovalRequest, now_ms: u64, opts: CardOptions) -> Self {
        let persona = persona_of(&req.holder);
        let v = |s: &str| sanitize(s, MAX_VALUE_CHARS);
        let mut details = vec![("Kto".to_owned(), persona.name.clone())];
        let mut plan = Vec::new();
        let mut options = vec![DecisionOption::Deny, DecisionOption::AllowOnce];
        let title = match &req.subject {
            ApprovalSubject::Action { capability, tool } => {
                details.push(("Co".into(), verb(capability).into()));
                details.push(("Narzędzie".into(), v(tool)));
                details.push(("Zakres".into(), v(&capability.to_string())));
                if req.grantable {
                    let until_ms = now_ms.saturating_add(opts.grant_ms.min(GRANT_MAX_MS));
                    options.push(DecisionOption::AllowInScope {
                        scope: capability.clone(),
                        until_ms,
                    });
                }
                format!("{} prosi o zgodę: {}", persona.name, verb(capability))
            }
            ApprovalSubject::Plan { title, steps } => {
                plan = steps
                    .iter()
                    .map(|s| PlanLine {
                        description: v(&s.description),
                        capability: v(&s.capability.to_string()),
                        risk: s.risk,
                    })
                    .collect();
                for (i, s) in plan.iter().take(MAX_PLAN_LINES).enumerate() {
                    let line = format!(
                        "{} — {} (ryzyko {})",
                        s.description,
                        s.capability,
                        s.risk.label_pl()
                    );
                    details.push((format!("Krok {}", i + 1), v(&line)));
                }
                if plan.len() > MAX_PLAN_LINES {
                    let rest = plan.len() - MAX_PLAN_LINES;
                    details.push(("…".into(), format!("i jeszcze {rest} kroków")));
                }
                format!("{}: plan do zatwierdzenia — {}", persona.name, v(title))
            }
            ApprovalSubject::Autonomy {
                target,
                from,
                to,
                until_ms,
            } => {
                details.push(("Dotyczy".into(), target_pl(target)));
                details.push((
                    "Zmiana".into(),
                    format!("{} → {}", level_pl(*from), level_pl(*to)),
                ));
                if let Some(until) = until_ms {
                    let left = time_left_pl(until.saturating_sub(now_ms));
                    details.push(("Na czas".into(), left));
                }
                "Zmiana poziomu autonomii".to_owned()
            }
            ApprovalSubject::Policy { policy } => {
                details.push(("Nowa polityka".into(), v(&policy_pl(policy))));
                "Zmiana polityk Jądra".to_owned()
            }
        };
        let risk_line = format!(
            "{} — {}",
            req.risk.label_pl(),
            reversible_pl(req.reversible)
        );
        details.push(("Ryzyko".into(), risk_line));
        details.push(("Źródło polecenia".into(), origin_pl(&req.origin).into()));
        if req.tainted {
            details.push(("Uwaga".into(), "sesja widziała niezaufaną treść".into()));
        }
        details.push(("Dlaczego pytam".into(), v(&req.explanation)));
        let left = time_left_pl(req.expires_at_ms.saturating_sub(now_ms));
        details.push(("Wygasa za".into(), left));
        let read_aloud = req.origin.is_voice().then(|| {
            format!(
                "{}. Potwierdź w oknie Brokera kliknięciem.",
                sanitize(&title, 200)
            )
        });
        Self {
            id: req.id,
            persona,
            title: sanitize(&title, MAX_VALUE_CHARS),
            details,
            risk: req.risk,
            reversible: req.reversible,
            voice_origin: req.origin.is_voice(),
            tainted: req.tainted,
            plan,
            options,
            read_aloud,
            hello_required: req.hello_required,
            created_at_ms: req.created_at_ms,
            expires_at_ms: req.expires_at_ms,
        }
    }
}
