//! Ocena trudności zadania z treści (sygnały przeniesione z Delty, `router_brain.py`, i rozwinięte):
//! długość, kod, prośba o rozumowanie, wiele kroków, obraz, liczba wierszy i pytań, kod we
//! wcześniejszej części rozmowy. Wynik 0–100; progi zależą od strategii (oszczędnie,
//! zrównoważenie, jakość). Bez sieci i bez modelu — deterministycznie, poniżej milisekundy.
//!
//! Polskie słowa są dopasowywane także bez ogonków („wyjasnij” = „wyjaśnij”) i po początku
//! wyrazu („przeanalizuj”, „przeanalizujesz”).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::message::{ContentBlock, Message, Role};
use crate::request::ChatRequest;
use crate::route_hint::Difficulty;

/// Strategia: jak łatwo zadanie przechodzi na mocniejszy model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    /// Oszczędnie — mocny model tylko przy wyraźnie trudnych zadaniach.
    Economy,
    /// Zrównoważenie (domyślnie).
    #[default]
    Balanced,
    /// Jakość — mocniejszy model już przy średnich zadaniach.
    Quality,
}

impl Strategy {
    /// Progi wyniku: (od średniego, od trudnego).
    pub fn thresholds(self) -> (u8, u8) {
        match self {
            Self::Economy => (45, 75),
            Self::Balanced => (28, 58),
            Self::Quality => (18, 42),
        }
    }

    /// Poziom trudności dla wyniku.
    pub fn level(self, score: u8) -> Difficulty {
        let (medium, hard) = self.thresholds();
        if score >= hard {
            Difficulty::Hard
        } else if score >= medium {
            Difficulty::Medium
        } else {
            Difficulty::Easy
        }
    }
}

/// Sygnał trudności (powód oceny pokazywany użytkownikowi).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    /// Kod w prośbie.
    Code,
    /// Prośba o analizę, projekt, dowód, naprawę.
    Reasoning,
    /// Kilka kroków naraz.
    MultiStep,
    /// Obraz do obejrzenia.
    Image,
    /// Długa treść.
    Long,
    /// Wiele wierszy.
    ManyLines,
    /// Kilka pytań naraz.
    Questions,
    /// Krótka prośba w rozmowie o kodzie (np. „popraw to”).
    CodeContext,
}

impl Signal {
    /// Waga sygnału w wyniku 0–100 (długość ma wagę zmienną, do 25).
    fn weight(self) -> u8 {
        match self {
            Self::Code => 30,
            Self::Reasoning => 32,
            Self::MultiStep => 26,
            Self::Image => 28,
            Self::Long => 0,
            Self::ManyLines => 12,
            Self::Questions => 8,
            Self::CodeContext => 28,
        }
    }

    /// Nazwa dla człowieka.
    pub fn label(self) -> &'static str {
        match self {
            Self::Code => "kod",
            Self::Reasoning => "rozumowanie",
            Self::MultiStep => "wiele kroków",
            Self::Image => "obraz",
            Self::Long => "długa treść",
            Self::ManyLines => "wiele wierszy",
            Self::Questions => "kilka pytań",
            Self::CodeContext => "rozmowa o kodzie",
        }
    }
}

/// Wynik oceny.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Estimate {
    /// Wynik 0–100.
    pub score: u8,
    /// Wykryte sygnały (w kolejności wag).
    pub signals: Vec<Signal>,
}

impl Estimate {
    /// Poziom trudności przy strategii.
    pub fn level(&self, strategy: Strategy) -> Difficulty {
        strategy.level(self.score)
    }

    /// Powody po ludzku („kod, rozumowanie”); bez sygnałów — „krótka, prosta prośba”.
    pub fn reasons(&self) -> String {
        if self.signals.is_empty() {
            return "krótka, prosta prośba".to_owned();
        }
        self.signals
            .iter()
            .map(|s| s.label())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

const CODE_MARKERS: [&str; 16] = [
    "```",
    "def ",
    "fn ",
    "function ",
    "class ",
    "import ",
    "select ",
    "async ",
    "() =>",
    "#include",
    "public static",
    "let mut ",
    "console.log",
    "</div>",
    "std::",
    "#!/",
];

/// Początki wyrazów prośby o rozumowanie (bez ogonków, małymi literami).
const REASONING_STEMS: [&str; 42] = [
    "przeanaliz",
    "analiz",
    "wyjasn",
    "wytlumacz",
    "porownaj",
    "porownan",
    "udowodn",
    "uzasadn",
    "zaprojektuj",
    "zaplanuj",
    "zoptymalizuj",
    "optymaliz",
    "zrefaktor",
    "refaktor",
    "debug",
    "zdebug",
    "napraw",
    "zaimplementuj",
    "implementac",
    "algorytm",
    "architektur",
    "strategi",
    "kompleksow",
    "zbadaj",
    "rozwiaz",
    "wyprowadz",
    "przemysl",
    "analyz",
    "explain",
    "compare",
    "prove",
    "design",
    "optimi",
    "refactor",
    "implement",
    "algorithm",
    "architect",
    "strategy",
    "reasoning",
    "derive",
    "evaluate",
    "solve",
];

/// Wyrazy, które zaczynają się jak prośba o rozumowanie, a nią nie są („naprawdę”).
const NOT_STEMS: [&str; 2] = ["naprawd", "designer"];

/// Frazy rozumowania (dopasowanie w tekście bez ogonków).
const REASONING_PHRASES: [&str; 3] = ["krok po kroku", "step by step", "plusy i minusy"];

/// Początki wyrazów wieloetapowości.
const MULTISTEP_STEMS: [&str; 7] = [
    "nastepnie",
    "potem",
    "najpierw",
    "wreszcie",
    "finally",
    "afterwards",
    "kolejno",
];

/// Frazy wieloetapowości.
const MULTISTEP_PHRASES: [&str; 6] = [
    "a potem",
    "po pierwsze",
    "po drugie",
    "after that",
    "first,",
    "and then",
];

/// Krótkie prośby, które w rozmowie o kodzie dotyczą kodu.
const FOLLOW_UP_STEMS: [&str; 10] = [
    "popraw", "zmien", "dodaj", "usun", "przepisz", "dalej", "fix", "change", "add", "continue",
];

/// Tekst małymi literami, bez polskich ogonków.
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            other => other,
        })
        .collect()
}

fn words(folded: &str) -> impl Iterator<Item = &str> {
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

fn any_stem(folded: &str, stems: &[&str]) -> bool {
    words(folded).any(|w| {
        stems.iter().any(|s| w.starts_with(s)) && !NOT_STEMS.iter().any(|n| w.starts_with(n))
    })
}

fn has_code(text: &str) -> bool {
    let low = text.to_lowercase();
    CODE_MARKERS.iter().any(|m| low.contains(m))
}

/// Numerowana lista kroków („1.” i „2.” na początku wierszy).
fn numbered_steps(text: &str) -> bool {
    let starts = |n: &str| {
        text.lines()
            .any(|l| l.trim_start().starts_with(&format!("{n}.")))
    };
    starts("1") && starts("2")
}

/// Ocena tekstu prośby. `image` — prośba ma obraz; `code_context` — wcześniej w rozmowie był kod.
pub fn estimate_text(text: &str, image: bool, code_context: bool) -> Estimate {
    let folded = fold(text);
    let mut signals = Vec::new();
    let code = has_code(text);
    if code {
        signals.push(Signal::Code);
    }
    if any_stem(&folded, &REASONING_STEMS) || REASONING_PHRASES.iter().any(|p| folded.contains(p)) {
        signals.push(Signal::Reasoning);
    }
    if image {
        signals.push(Signal::Image);
    }
    if !code && code_context && any_stem(&folded, &FOLLOW_UP_STEMS) {
        signals.push(Signal::CodeContext);
    }
    if any_stem(&folded, &MULTISTEP_STEMS)
        || MULTISTEP_PHRASES.iter().any(|p| folded.contains(p))
        || numbered_steps(text)
    {
        signals.push(Signal::MultiStep);
    }
    if text.lines().count() > 5 {
        signals.push(Signal::ManyLines);
    }
    if text.matches('?').count() >= 2 {
        signals.push(Signal::Questions);
    }
    let chars = text.chars().count();
    let length = u8::try_from((chars.min(600) * 25) / 600).unwrap_or(25);
    if chars >= 300 {
        signals.push(Signal::Long);
    }
    let sum: u32 = signals.iter().map(|s| u32::from(s.weight())).sum::<u32>() + u32::from(length);
    Estimate {
        score: u8::try_from(sum.min(100)).unwrap_or(100),
        signals,
    }
}

fn texts(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ocena żądania: ostatnia wiadomość użytkownika z tekstem (wyniki narzędzi pomijane), obraz
/// w niej i kod w czterech wcześniejszych wiadomościach rozmowy.
pub fn estimate_request(req: &ChatRequest) -> Estimate {
    let last = req
        .messages
        .iter()
        .rposition(|m| m.role == Role::User && !texts(m).trim().is_empty());
    let Some(at) = last else {
        return estimate_text("", false, false);
    };
    let message = &req.messages[at];
    let image = message
        .content
        .iter()
        .any(|b| matches!(b, ContentBlock::Image { .. }));
    let code_context = req.messages[at.saturating_sub(4)..at]
        .iter()
        .any(|m| has_code(&texts(m)));
    estimate_text(&texts(message), image, code_context)
}

#[cfg(test)]
#[path = "difficulty_tests.rs"]
mod tests;
