//! Rozstrzyganie adresatki wypowiedzi: „Beta, …”, „hej Gama”, „Delto”, `@alfa`.
//! Zwrot po imieniu zawsze wygrywa; bez imienia odpowiada Dyrygentka obsady.

use std::collections::BTreeSet;

use crate::cast::Cast;
use crate::ids::PersonaId;
use crate::model::{Case, Persona};
use crate::text::{Token, tokenize};

/// Wykrzykniki / zwroty przed imieniem („hej Alfa”, „słuchaj Beto”, „dzień dobry Gamo”).
const INTERJECTIONS: [&str; 16] = [
    "hej", "hejka", "hey", "hi", "halo", "hallo", "ej", "sluchaj", "czesc", "witaj", "ok", "okej",
    "okay", "dzien", "dobry", "prosze",
];

/// Wzmianka o personie w wypowiedzi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    /// Persona.
    pub persona: PersonaId,
    /// Indeks słowa w `tokenize(text)`.
    pub index: usize,
    /// Przypadki pasujące do formy (np. „Delcie” = celownik i miejscownik).
    pub cases: BTreeSet<Case>,
}

impl Mention {
    /// Forma, którą można się zwracać (mianownik lub wołacz).
    pub fn is_address_form(&self) -> bool {
        self.cases.contains(&Case::Nominative) || self.cases.contains(&Case::Vocative)
    }

    /// Forma jednoznacznie wołacza („Delto”, „Alfo”).
    pub fn is_vocative_only(&self) -> bool {
        self.cases.contains(&Case::Vocative) && !self.cases.contains(&Case::Nominative)
    }
}

/// Wszystkie wzmianki o personach (każda forma odmiany imienia) w kolejności słów.
pub fn find_mentions(tokens: &[Token], personas: &[Persona]) -> Vec<Mention> {
    tokens
        .iter()
        .enumerate()
        .flat_map(|(index, token)| {
            personas.iter().filter_map(move |p| {
                let cases = p.forms.cases_of(&token.word);
                (!cases.is_empty()).then(|| Mention {
                    persona: p.id.clone(),
                    index,
                    cases,
                })
            })
        })
        .collect()
}

/// Indeksy słów otwierających zdanie (po pominięciu wykrzykników).
fn sentence_heads(tokens: &[Token]) -> Vec<usize> {
    let mut heads = Vec::new();
    let mut at_start = true;
    for (i, token) in tokens.iter().enumerate() {
        if at_start && !INTERJECTIONS.contains(&token.word.as_str()) {
            heads.push(i);
            at_start = false;
        }
        if token.ends_sentence {
            at_start = true;
        }
    }
    heads
}

/// Jawna adresatka w słowach wypowiedzi (bez zastępstwa Dyrygentką).
pub(crate) fn addressee_in(tokens: &[Token], mentions: &[Mention]) -> Option<PersonaId> {
    // 1. `@imię` z composera.
    if let Some(m) = mentions.iter().find(|m| tokens[m.index].at) {
        return Some(m.persona.clone());
    }
    // 2. Imię (mianownik/wołacz) na początku zdania, także po „hej”, „słuchaj”…
    let heads = sentence_heads(tokens);
    if let Some(m) = mentions
        .iter()
        .find(|m| m.is_address_form() && heads.contains(&m.index))
    {
        return Some(m.persona.clone());
    }
    // 3. Jednoznaczny wołacz w dowolnym miejscu („Powiedz mi, Delto, …”).
    if let Some(m) = mentions.iter().find(|m| m.is_vocative_only()) {
        return Some(m.persona.clone());
    }
    // 4. Wtrącenie: „…, Delta?” / „…, Delta, …”.
    mentions
        .iter()
        .find(|m| {
            let t = &tokens[m.index];
            m.is_address_form() && t.comma_before && (t.comma_after || t.ends_sentence)
        })
        .map(|m| m.persona.clone())
}

/// Jawna adresatka wypowiedzi albo `None`, gdy nikt nie jest wywołany po imieniu.
pub fn parse_addressee(text: &str, personas: &[Persona]) -> Option<PersonaId> {
    let tokens = tokenize(text);
    let mentions = find_mentions(&tokens, personas);
    addressee_in(&tokens, &mentions)
}

/// Adresatka: imię wygrywa, inaczej Dyrygentka obsady.
pub fn resolve_addressee(text: &str, personas: &[Persona], cast: &Cast) -> Option<PersonaId> {
    parse_addressee(text, personas).or_else(|| cast.conductor())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::builtin_personas;

    fn who(text: &str) -> Option<String> {
        parse_addressee(text, &builtin_personas()).map(|p| p.0)
    }

    #[test]
    fn addressing_table() {
        let cases: [(&str, Option<&str>); 22] = [
            ("Beta, co masz na dziś?", Some("beta")),
            ("hej Gama, sprawdź to", Some("gama")),
            ("Hej Alfa", Some("alfa")),
            ("Delto, otwórz plik", Some("delta")),
            ("Słuchaj Beto, jak idzie?", Some("beta")),
            ("Powiedz mi, Delto, która godzina", Some("delta")),
            ("Która godzina, Gama?", Some("gama")),
            ("@delta uruchom testy", Some("delta")),
            ("Dzięki. Alfo, podsumuj.", Some("alfa")),
            ("Alfa, zapytaj Betę o plan", Some("alfa")),
            ("Dzień dobry Gamo", Some("gama")),
            ("zazolc Beto", Some("beta")),
            ("DELTA: status", Some("delta")),
            ("Alfo", Some("alfa")),
            ("Co myśli o tym Gama", None),
            ("Przekaż to Delcie", None),
            ("Zapytaj Bety o zdanie", None),
            ("Jaka jest pogoda?", None),
            ("alfabet i betonowa ściana", None),
            ("", None),
            ("hej", None),
            ("To jest wersja beta programu", None),
        ];
        for (text, expected) in cases {
            assert_eq!(who(text).as_deref(), expected, "{text}");
        }
    }
}
