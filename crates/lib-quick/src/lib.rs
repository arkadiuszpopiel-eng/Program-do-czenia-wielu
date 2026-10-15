//! Polecenia wykonywane bez modelu (plan v6, P5): rachunki, literowanie, losowanie, data
//! i godzina, rachunki na datach, przeliczanie jednostek. Odpowiedź jest deterministyczna (poza losowaniem), natychmiastowa i działa offline.
//!
//! Zasada dopasowania: całe polecenie (po normalizacji) musi pasować do wzorca albo — w rachunkach
//! — każde słowo musi być liczbą, działaniem albo wypełniaczem. Wszystko inne idzie do modelu,
//! więc „ile kosztuje 5 jabłek” czy „która godzina w Tokio” nie dostaną odpowiedzi z tej
//! biblioteki. Biblioteka nie wykonuje żadnych akcji — tylko liczy tekst odpowiedzi.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod clock;
pub mod custom;
mod dates;

/// Klucze konfiguracji poleceń (wspólne dla rdzenia i silnika rozmowy).
pub mod keys {
    /// Polecenia bez modelu włączone (domyślnie tak).
    pub const ENABLED: &str = "sessions.local_commands";
    /// Własne polecenia użytkownika (tekst JSON).
    pub const CUSTOM: &str = "sessions.custom_commands";
}
mod math;
pub mod quiz;
mod random;
mod spell;
pub mod text;
mod units;
mod words;

use chrono::NaiveDateTime;

/// Rodzaj polecenia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Rachunek.
    Math,
    /// Literowanie słowa.
    Spell,
    /// Kostka, moneta, liczba losowa.
    Random,
    /// Data albo godzina.
    Clock,
    /// Quiz z tabliczki mnożenia (stan trzyma wywołujący — [`quiz`]).
    Quiz,
    /// Własne polecenie użytkownika ([`custom`]).
    Custom,
    /// Rachunek na datach: dni do daty, dzień tygodnia, data za N dni.
    Dates,
    /// Przeliczanie jednostek.
    Units,
}

impl Command {
    /// Nazwa po polsku (oś czasu, dziennik).
    pub fn label(self) -> &'static str {
        match self {
            Self::Math => "rachunek",
            Self::Spell => "literowanie",
            Self::Random => "losowanie",
            Self::Clock => "data i godzina",
            Self::Quiz => "quiz",
            Self::Custom => "własne polecenie",
            Self::Dates => "rachunek na datach",
            Self::Units => "przeliczanie jednostek",
        }
    }
}

/// Odpowiedź polecenia (Markdown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// Polecenie.
    pub command: Command,
    /// Tekst odpowiedzi.
    pub text: String,
}

/// Pozycja katalogu (opis i przykłady — test sprawdza, że każdy przykład trafia w polecenie).
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// Polecenie.
    pub command: Command,
    /// Opis po polsku.
    pub title: &'static str,
    /// Przykładowe wiadomości.
    pub examples: &'static [&'static str],
}

/// Katalog wbudowanych poleceń.
pub const CATALOG: &[Entry] = &[
    Entry {
        command: Command::Math,
        title: "Rachunki: działania, procenty, potęgi, pierwiastki, liczby słowami",
        examples: &[
            "Ile to jest 7 razy 8?",
            "oblicz (2+3)*4",
            "20% z 150",
            "pierwiastek z 81",
            "dwadzieścia trzy plus sto",
            "2 do potęgi 10",
            "ile to jest 7 podzielone przez 2",
        ],
    },
    Entry {
        command: Command::Spell,
        title: "Literowanie jednego słowa",
        examples: &[
            "Przeliteruj rower",
            "jak się literuje słowo żółw",
            "jak się pisze wyraz źdźbło",
        ],
    },
    Entry {
        command: Command::Random,
        title: "Kostka, moneta, liczba losowa",
        examples: &[
            "Rzuć kostką",
            "rzuć dwiema kostkami",
            "orzeł czy reszka?",
            "wylosuj liczbę od 1 do 10",
            "podaj losową cyfrę",
        ],
    },
    Entry {
        command: Command::Clock,
        title: "Data i godzina z zegara komputera",
        examples: &[
            "Która godzina?",
            "Hej Alfa, jaki dziś dzień?",
            "jaka jest data",
            "którego dzisiaj",
        ],
    },
    Entry {
        command: Command::Quiz,
        title: "Quiz z tabliczki mnożenia (10 pytań, „koniec” przerywa)",
        examples: &[
            "Przepytaj mnie z tabliczki mnożenia",
            "sprawdź mnie z tabliczki mnożenia przez 7",
            "tabliczka mnożenia przez 3",
        ],
    },
    Entry {
        command: Command::Dates,
        title: "Rachunki na datach: dni do daty i od daty, dzień tygodnia, data za N dni",
        examples: &[
            "Ile dni do 24 grudnia?",
            "ile dni minęło od 1 września",
            "Jaki dzień tygodnia był 1 stycznia 2000?",
            "jaka data będzie za 30 dni",
            "jaki dzień będzie jutro",
        ],
    },
    Entry {
        command: Command::Units,
        title: "Przeliczanie jednostek: długość, masa, objętość, temperatura, prędkość, czas",
        examples: &[
            "Ile to 5 km w milach?",
            "zamień 30 stopni Celsjusza na Fahrenheita",
            "ile cm ma cal",
            "100 km/h na m/s",
            "2,5 kg w funtach",
        ],
    },
];

/// Najdłuższa wiadomość, którą biblioteka w ogóle rozważa (dłuższe idą do modelu).
pub const MAX_CHARS: usize = 160;

/// Odpowiedź bez modelu albo `None` (wiadomość idzie do modelu). `now` — czas lokalny,
/// `roll(n)` — liczba losowa z zakresu `0..n`.
pub fn answer(
    message: &str,
    now: NaiveDateTime,
    roll: &mut dyn FnMut(u64) -> u64,
) -> Option<Reply> {
    if message.chars().count() > MAX_CHARS || message.contains('\n') {
        return None;
    }
    let text = text::normalize(message);
    if text.is_empty() {
        return None;
    }
    let reply = |command, text| Some(Reply { command, text });
    if let Some(t) = clock::answer(&text, now) {
        return reply(Command::Clock, t);
    }
    if let Some(t) = dates::answer(&text, now) {
        return reply(Command::Dates, t);
    }
    if let Some(t) = spell::answer(message, &text) {
        return reply(Command::Spell, t);
    }
    if let Some(t) = random::answer(&text, roll) {
        return reply(Command::Random, t);
    }
    if let Some(t) = units::answer(&text) {
        return reply(Command::Units, t);
    }
    math::answer(&text).and_then(|t| reply(Command::Math, t))
}

/// Czy wiadomość dostanie odpowiedź bez modelu (bez losowania i bez zegara).
pub fn matches(message: &str) -> bool {
    answer(message, NaiveDateTime::default(), &mut |_| 0).is_some()
}
