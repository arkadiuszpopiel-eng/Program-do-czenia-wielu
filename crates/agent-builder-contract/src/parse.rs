//! Rozmowa → szkic (PLAN §9.5 „opis słowami → manifest”): deterministyczna ekstrakcja z opisu
//! PL (imię, zadania, grupy narzędzi ze słów kluczowych, zakresy zapisu, charakter, głos,
//! autonomia) + pytania o braki; opcjonalny model ([`DraftLlm`]) tylko **uzupełnia** braki —
//! autonomia zawsze niższa z dwóch, a szkic i tak przechodzi pełną walidację polityki.

use async_trait::async_trait;
use personas_contract::fold;
use risk_classifier_contract::AutonomyLevel;

use crate::draft::{AgentDraft, DraftProposal, LimitsDraft, RoleDraft, VoiceDraft};

/// Port do modelu: szkic z opisu (dane niezaufane — walidowane jak formularz).
#[async_trait]
pub trait DraftLlm: Send + Sync {
    /// Szkic z opisu właściciela.
    async fn draft(&self, description: &str) -> Result<AgentDraft, String>;
}

const NAME_CUES: [&str; 7] = [
    "agentke", "agentka", "agentki", "imieniu", "imie", "nazywa", "nazwij",
];
const CHARACTER: [&str; 10] = [
    "spokojna",
    "energiczna",
    "ciepła",
    "rzeczowa",
    "pogodna",
    "zwięzła",
    "dokładna",
    "cierpliwa",
    "konkretna",
    "dociekliwa",
];

/// (fragment po `fold`, grupy narzędzi, zakres zapisu).
const KEYWORDS: [(&str, &[&str], Option<&str>); 14] = [
    ("pobran", &["fs"], Some("%USERPROFILE%\\Downloads\\**")),
    ("dokument", &["fs"], Some("%USERPROFILE%\\Documents\\**")),
    ("pulpit", &["fs"], Some("%USERPROFILE%\\Desktop\\**")),
    ("zdjec", &["fs"], Some("%USERPROFILE%\\Pictures\\**")),
    ("plik", &["fs"], None),
    ("folder", &["fs"], None),
    ("katalog", &["fs"], None),
    ("polecen", &["shell"], None),
    ("terminal", &["shell"], None),
    ("kod", &["worktree", "shell"], None),
    ("internet", &["web", "browser"], None),
    ("stron", &["web", "browser"], None),
    ("notatk", &["memory"], None),
    ("okn", &["gui.control"], None),
];

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

fn name_of(text: &str) -> Option<String> {
    let ws = words(text);
    ws.windows(2).find_map(|w| {
        let cue = fold(&w[0]);
        let cand = &w[1];
        (NAME_CUES.iter().any(|c| cue.starts_with(c))
            && cand.chars().next().is_some_and(char::is_uppercase))
        .then(|| cand.clone())
    })
}

fn duty_of(text: &str) -> Option<String> {
    let f = fold(text);
    let start = ["ktora ", "zeby ", "aby "]
        .iter()
        .filter_map(|c| f.find(c).map(|i| i + c.len()))
        .min()?;
    let original: String = text.chars().skip(f[..start].chars().count()).collect();
    let end = original
        .find(['.', '!', '?', ';'])
        .unwrap_or(original.len());
    let duty = original[..end].trim().trim_end_matches(',').trim();
    (!duty.is_empty()).then(|| duty.chars().take(300).collect())
}

fn autonomy_of(f: &str) -> Option<AutonomyLevel> {
    let ws = words(f);
    if f.contains("bez pytania")
        || f.contains("na maksa")
        || ws.iter().any(|w| w == "maks" || w == "l4")
    {
        return Some(AutonomyLevel::L4);
    }
    if f.contains("pyta o wszystko") {
        return Some(AutonomyLevel::L1);
    }
    if f.contains("pyta o ryzykowne") {
        return Some(AutonomyLevel::L2);
    }
    (f.contains("samodzielnie") || f.contains("sama ")).then_some(AutonomyLevel::L3)
}

fn slug(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in fold(text).chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= max {
            break;
        }
    }
    out.trim_end_matches('-').to_owned()
}

/// Szkic z opisu (deterministycznie) + pytania o braki.
pub fn from_description(text: &str) -> DraftProposal {
    let f = fold(text);
    let mut q = Vec::new();
    let name = name_of(text);
    if name.is_none() {
        q.push("Jak ma mieć na imię nowa agentka?".to_owned());
    }
    let read_only = [
        "tylko czyta",
        "tylko odczyt",
        "nic nie zmienia",
        "tylko sprawdza",
    ]
    .iter()
    .any(|p| f.contains(p));
    let mut groups: Vec<String> = Vec::new();
    let mut scopes: Vec<String> = Vec::new();
    for (kw, gs, scope) in KEYWORDS {
        if f.contains(kw) {
            for g in gs {
                let g = if read_only && *g == "fs" {
                    "fs.read"
                } else {
                    g
                };
                if !groups.iter().any(|x| x == g) {
                    groups.push(g.to_owned());
                }
            }
            if let Some(s) = scope.filter(|_| !read_only) {
                scopes.push(s.to_owned());
            }
        }
    }
    let character: Vec<&str> = CHARACTER
        .iter()
        .copied()
        .filter(|c| f.contains(&fold(c)))
        .collect();
    if character.is_empty() {
        q.push("Jaki ma mieć charakter (np. spokojna, energiczna, rzeczowa)?".to_owned());
    }
    let autonomy = autonomy_of(&f);
    if autonomy == Some(AutonomyLevel::L4) {
        q.push("Kreator nie podnosi autonomii: najwyżej poziom tej sesji (maks. L3). L4 włączysz tylko w Ustawieniach.".to_owned());
    }
    let pitch = if f.contains("nizsz") || f.contains("nisk") {
        0.95
    } else if f.contains("wyzsz") || f.contains("wysok") {
        1.09
    } else {
        0.0
    };
    let rate = if f.contains("szybk") || f.contains("zwaw") {
        1.1
    } else if f.contains("woln") {
        0.92
    } else {
        0.0
    };
    let voice = (pitch > 0.0 || rate > 0.0).then(|| VoiceDraft {
        base: "pl-f2".into(),
        pitch: if pitch > 0.0 { pitch } else { 1.02 },
        rate: if rate > 0.0 { rate } else { 1.0 },
        perceived_age: 22,
        timbre: if pitch < 1.0 && pitch > 0.0 {
            "niższa, miękka".into()
        } else {
            "jasna, wyraźna".into()
        },
        design_prompt: "młoda dorosła kobieta, czysta polszczyzna".into(),
    });
    if voice.is_none() {
        q.push("Głos dobiorę automatycznie (odrębny od pozostałych) — wolisz niższy, wyższy albo szybszy?".to_owned());
    }
    let duty = duty_of(text);
    let role = duty.as_ref().map(|d| {
        let model_policy = if groups.iter().any(|g| g == "worktree") {
            "code"
        } else if groups.iter().any(|g| g == "web") {
            "research"
        } else if groups.iter().any(|g| g == "gui.control") {
            "gui-vision"
        } else {
            "conversation"
        };
        let title: String = d.chars().take(60).collect();
        RoleDraft {
            id: slug(d, 32),
            name: title.clone(),
            description: d.clone(),
            prompt: format!("Twoje zadanie: {d}. Pracujesz starannie, opisujesz plan przed działaniem i raportujesz wynik; o sobie mówisz w rodzaju żeńskim („zrobiłam”)."),
            model_policy: model_policy.into(),
            tools: groups.clone(),
            read_only,
            untrusted_isolated: groups.iter().any(|g| g == "web"),
            author: !read_only && !groups.is_empty(),
        }
    });
    if role.is_none() {
        q.push("Czym ma się zajmować (np. „sortuje folder Pobrane według typu”)?".to_owned());
    }
    let draft = AgentDraft {
        name,
        character: (!character.is_empty()).then(|| character.join(", ")),
        voice,
        role,
        limits: LimitsDraft {
            autonomy,
            fs_write: scopes,
            ..LimitsDraft::default()
        },
        ..AgentDraft::default()
    };
    DraftProposal {
        draft,
        questions: q,
    }
}

/// Szkic z rozmowy z modelem: model uzupełnia tylko braki; autonomia = niższa z dwóch;
/// błąd modelu = szkic deterministyczny.
pub async fn from_conversation(text: &str, llm: &dyn DraftLlm) -> DraftProposal {
    let mut p = from_description(text);
    let Ok(m) = llm.draft(text).await else {
        return p;
    };
    let d = &mut p.draft;
    d.name = d.name.take().or(m.name);
    d.character = d.character.take().or(m.character);
    d.voice = d.voice.take().or(m.voice);
    d.role = d.role.take().or(m.role);
    d.limits.autonomy = match (d.limits.autonomy, m.limits.autonomy) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    if d.limits.fs_write.is_empty() {
        d.limits.fs_write = m.limits.fs_write;
    }
    p.questions.retain(|q| {
        !(q.starts_with("Jak ma") && d.name.is_some()
            || q.starts_with("Jaki ma") && d.character.is_some()
            || q.starts_with("Czym ma") && d.role.is_some()
            || q.starts_with("Głos") && d.voice.is_some())
    });
    p
}
