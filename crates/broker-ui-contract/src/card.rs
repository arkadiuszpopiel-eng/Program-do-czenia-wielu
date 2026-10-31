//! Model prezentacji prośby o zatwierdzenie (karta Broker-UI, makieta 15): kto, co, zakres,
//! ryzyko, odwracalność, źródło polecenia, plan wielu akcji, opcje decyzji i czas do wygaśnięcia.
//! Cały tekst jest czystym tekstem po sanityzacji (bez znaków sterujących, bidi i zerowej
//! szerokości), obcięty do limitu — Broker-UI niczego nie renderuje jako HTML/markdown.

use risk_classifier_contract::{AutonomyLevel, Reversibility, RiskLevel};
use safety_broker_contract::{ApprovalId, ApprovalRequest, ApprovalSubject, Capability, Holder};

use crate::words::{
    asked_at_pl, mode_name_pl, mode_note_pl, origin_pl, place_pl, policy_pl, reversible_pl,
    target_pl, verb,
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
    /// Zmiana trybu pracy: tryb docelowy. Okno pokazuje wtedy nazwę trybu z menu w pasku Alfy,
    /// przycisk „Włącz: …” i nie odlicza czasu — wybór zostaje zapamiętany (właściciel 27.10).
    #[serde(default)]
    pub mode_change: Option<AutonomyLevel>,
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
        let mut technical = None;
        let title = match &req.subject {
            ApprovalSubject::Action {
                capability,
                tool,
                summary,
            } => {
                // Najpierw konkret od narzędzia („trwałe usunięcie „a.txt””), potem rodzaj
                // i miejsce po ludzku; nazwa techniczna narzędzia na końcu karty.
                let what = summary.as_deref().map(v).filter(|s| !s.is_empty());
                details.clear();
                details.push((
                    "Co dokładnie".into(),
                    what.clone().unwrap_or_else(|| verb(capability).into()),
                ));
                details.push(("Kto".into(), persona.name.clone()));
                details.push(("Rodzaj".into(), verb(capability).into()));
                details.push(("Gdzie".into(), place_pl(capability)));
                technical = Some(v(tool));
                if req.grantable {
                    let until_ms = now_ms.saturating_add(opts.grant_ms.min(GRANT_MAX_MS));
                    options.push(DecisionOption::AllowInScope {
                        scope: capability.clone(),
                        until_ms,
                    });
                }
                let short = what.filter(|w| w.chars().count() <= 90);
                format!(
                    "{} prosi o zgodę: {}",
                    persona.name,
                    short.unwrap_or_else(|| verb(capability).into())
                )
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
                // Miejsce po ludzku zamiast zapisu zdolności (`fs.write(c:\…\**)`).
                for (i, s) in steps.iter().take(MAX_PLAN_LINES).enumerate() {
                    let line = format!(
                        "{} — {}: {} (ryzyko {})",
                        v(&s.description),
                        verb(&s.capability),
                        place_pl(&s.capability),
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
                // Prośbę zgłasza Jądro w imieniu właściciela — „Kto” nic tu nie mówi.
                details.clear();
                details.push((
                    "Tryb".into(),
                    format!("{} → {}", mode_name_pl(*from), mode_name_pl(*to)),
                ));
                details.push(("Dotyczy".into(), target_pl(target)));
                details.push(("Co się zmieni".into(), mode_note_pl(*to).into()));
                if *to == AutonomyLevel::L4 {
                    details.push((
                        "Zawsze zostaje".into(),
                        "STOP (Ctrl+Shift+F12), dziennik cofania i twarde blokady Jądra".into(),
                    ));
                }
                match until_ms {
                    Some(until) => {
                        let left = time_left_pl(until.saturating_sub(now_ms));
                        details.push(("Na czas".into(), left));
                    }
                    None => details.push((
                        "Na stałe".into(),
                        "zostaje po restarcie; zmienisz w menu „Tryb pracy”".into(),
                    )),
                }
                format!("Zmiana trybu pracy: {}", mode_name_pl(*to))
            }
            ApprovalSubject::Policy { policy } => {
                details.push(("Nowa polityka".into(), v(&policy_pl(policy))));
                "Zmiana polityk Jądra".to_owned()
            }
        };
        let mode_change = match &req.subject {
            ApprovalSubject::Autonomy { to, .. } => Some(*to),
            _ => None,
        };
        if mode_change.is_none() {
            let risk_line = format!(
                "{} — {}",
                req.risk.label_pl(),
                reversible_pl(req.reversible)
            );
            details.push(("Ryzyko".into(), risk_line));
        }
        details.push(("Źródło polecenia".into(), origin_pl(&req.origin).into()));
        if req.tainted {
            details.push(("Uwaga".into(), "sesja widziała niezaufaną treść".into()));
        }
        // Zmiana trybu: bez „Dlaczego pytam” (powtarzało wiersz „Tryb”). Bez odliczania na żadnej
        // karcie — tekst w oknie nie odświeża się co sekundę, więc licznik kłamał; prośba nadal
        // wygasa u Sędziego, a okno wtedy ją wycofuje (właściciel 27.10 i 31.10).
        if mode_change.is_none() {
            let mut why = v(&req.explanation);
            if let Some(level) = req.autonomy {
                why = format!("{why} {}", asked_at_pl(level));
            }
            details.push(("Dlaczego pytam".into(), why));
        }
        if let Some(tool) = technical {
            details.push(("Narzędzie (techn.)".into(), tool));
        }
        let read_aloud = req.origin.is_voice().then(|| {
            format!(
                "{}. Potwierdź w oknie Sędziego Cieni kliknięciem.",
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
            mode_change,
        }
    }
}
