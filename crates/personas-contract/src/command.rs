//! Polecenia zmiany obsady z tekstu/głosu (deterministyczny parser PL, bez LLM):
//! „Beta, teraz ty prowadzisz” → Beta dostaje Dyrygentkę; „Delta, przejmij weryfikację” → Krytyczka;
//! „Przekaż prowadzenie Delcie”; „Gama, przestań weryfikować”; „Obsada solo”; „Alfa, zrób wszystko sama”.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::address::{Mention, addressee_in, find_mentions};
use crate::cast::{Cast, CastError};
use crate::catalog::Catalog;
use crate::ids::{PersonaId, RoleId, TemplateId};
use crate::model::Case;
use crate::text::{Token, fold, is_question, tokenize};

/// Rozpoznane polecenie obsady.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum CastCommand {
    /// Przydziel role agentce; `exclusive` = przejęcie (odebranie pozostałym).
    Assign {
        /// Agentka.
        persona: PersonaId,
        /// Role.
        roles: Vec<RoleId>,
        /// Przejęcie roli (domyślnie) czy dodanie („ty też…”). Role unikalne zawsze przejmowane.
        exclusive: bool,
    },
    /// Odbierz role agentce („przestań weryfikować”, „już nie jesteś krytyczką”).
    Remove {
        /// Agentka.
        persona: PersonaId,
        /// Role.
        roles: Vec<RoleId>,
    },
    /// Zastosuj szablon obsady (Solo: dla wskazanej agentki albo obecnej Dyrygentki).
    Template {
        /// Szablon.
        template: TemplateId,
        /// Agentka dla „Solo”.
        persona: Option<PersonaId>,
    },
}

/// Słowa ról wbudowanych: (rola, prefiksy, słowa dokładne) — po `fold`.
const ROLE_WORDS: [(&str, &[&str], &[&str]); 9] = [
    (
        "conductor",
        &[
            "prowadz", "dyrygent", "dowodz", "koordyn", "kieruj", "kierown",
        ],
        &[],
    ),
    (
        "speaker",
        &["mowczyn", "rozmow"],
        &["mow", "mowisz", "mowi", "mowic", "glos"],
    ),
    (
        "thinker",
        &["myslicielk", "planist", "planow", "planuj"],
        &["myslisz", "mysli", "myslenie"],
    ),
    (
        "operator",
        &[
            "wykonawczyn",
            "operatork",
            "wykonywan",
            "komputer",
            "sterowan",
        ],
        &[],
    ),
    (
        "coder",
        &[
            "koderk",
            "kodow",
            "koduj",
            "programist",
            "programow",
            "programuj",
        ],
        &["kod", "kodem", "kodu"],
    ),
    (
        "critic",
        &["krytyczk", "weryfik", "recenz", "sprawdza"],
        &[],
    ),
    (
        "researcher",
        &[
            "badaczk",
            "badan",
            "badaj",
            "research",
            "wyszukiw",
            "przeszukiw",
        ],
        &[],
    ),
    (
        "keeper",
        &[
            "strazniczk",
            "pamiec",
            "organizac",
            "organizuj",
            "notuj",
            "notatk",
            "notowan",
        ],
        &[],
    ),
    (
        "writer",
        &["pisark", "tlumacz", "pisan", "piszesz"],
        &["pisz", "pisze"],
    ),
];

/// Znaczniki zmiany: przejęcie / stan („niech”, „zostań”, „jesteś teraz”…).
const STRONG_PREFIXES: [&str; 9] = [
    "przejm", "zostan", "zostaj", "bedzi", "obejm", "zajm", "dostaj", "dostan", "odtad",
];
const STRONG_WORDS: [&str; 4] = ["niech", "jestes", "jest", "badz"];
/// Znaczniki przekazania komuś (adresatka roli w celowniku/bierniku).
const GIVE_PREFIXES: [&str; 5] = ["przekaz", "odda", "przydziel", "przypisz", "wyznacz"];
const ADDITIVE: [&str; 4] = ["tez", "rowniez", "takze", "dodatkowo"];
const REMOVE_PREFIXES: [&str; 3] = ["przestan", "zwaln", "zwoln"];
const POLITE: [&str; 5] = ["mozesz", "moglabys", "mozecie", "moglybyscie", "prosze"];
const TEMPLATE_MARKERS: [&str; 3] = ["obsad", "szablon", "tryb"];

fn has_prefix(tokens: &[Token], prefixes: &[&str]) -> bool {
    tokens
        .iter()
        .any(|t| prefixes.iter().any(|p| t.word.starts_with(p)))
}

fn has_word(tokens: &[Token], words: &[&str]) -> bool {
    tokens.iter().any(|t| words.contains(&t.word.as_str()))
}

fn has_seq(tokens: &[Token], seq: &[&str]) -> bool {
    tokens
        .windows(seq.len())
        .any(|w| w.iter().zip(seq).all(|(t, s)| t.word == *s))
}

/// Role wskazane słowami (wbudowane z tabeli; własne — po początkach słów nazwy).
fn roles_in(tokens: &[Token], catalog: &Catalog) -> Vec<RoleId> {
    let mut roles: Vec<RoleId> = Vec::new();
    for role in catalog.roles() {
        let hit = match ROLE_WORDS.iter().find(|(id, _, _)| *id == role.id.as_str()) {
            Some((_, prefixes, exact)) => has_prefix(tokens, prefixes) || has_word(tokens, exact),
            None => {
                let stems: Vec<String> = fold(&role.name)
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| w.chars().count() >= 4)
                    .map(|w| w.chars().take(6).collect())
                    .collect();
                let stems: Vec<&str> = stems.iter().map(String::as_str).collect();
                has_prefix(tokens, &stems)
            }
        };
        if hit {
            roles.push(role.id.clone());
        }
    }
    roles
}

/// Szablon wskazany słowami (wbudowane z listy; własne po słowach nazwy lub id).
fn template_in(tokens: &[Token], catalog: &Catalog) -> Option<TemplateId> {
    let builtin: [(TemplateId, &[&str]); 4] = [
        (TemplateId::standard(), &["standard"]),
        (TemplateId::solo(), &["solo", "samodziel", "jednoosobow"]),
        (TemplateId::coding(), &["kodowan", "programowan"]),
        (TemplateId::research(), &["badani", "badawcz", "research"]),
    ];
    if let Some((id, _)) = builtin.iter().find(|(_, p)| has_prefix(tokens, p)) {
        return Some(id.clone());
    }
    catalog
        .templates()
        .iter()
        .filter(|t| !t.builtin)
        .find(|t| {
            let name = fold(&t.name);
            has_word(tokens, &[t.id.as_str()])
                || name
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| w.len() >= 3)
                    .any(|w| has_word(tokens, &[w]))
        })
        .map(|t| t.id.clone())
}

fn first_with(mentions: &[Mention], cases: &[Case]) -> Option<PersonaId> {
    mentions
        .iter()
        .find(|m| cases.iter().any(|c| m.cases.contains(c)))
        .map(|m| m.persona.clone())
}

/// Parsuje polecenie zmiany obsady; `None`, gdy tekst nim nie jest (zwykła prośba, pytanie).
pub fn parse_cast_command(text: &str, catalog: &Catalog, cast: &Cast) -> Option<CastCommand> {
    let tokens = tokenize(text);
    if is_question(text) && !has_word(&tokens, &POLITE) {
        return None;
    }
    let mentions = find_mentions(&tokens, catalog.personas());
    let addressee = addressee_in(&tokens, &mentions);
    let any_mention = || {
        addressee
            .clone()
            .or_else(|| mentions.first().map(|m| m.persona.clone()))
    };

    let solo_phrase = has_word(&tokens, &["solo"])
        || (has_word(&tokens, &["sama"]) && has_prefix(&tokens, &["wszystk", "pracuj"]));
    if (has_prefix(&tokens, &TEMPLATE_MARKERS) || solo_phrase)
        && let Some(template) = template_in(&tokens, catalog).or(solo_phrase.then(TemplateId::solo))
    {
        let persona = (template == TemplateId::solo())
            .then(|| any_mention().or_else(|| cast.conductor()))
            .flatten();
        return Some(CastCommand::Template { template, persona });
    }

    let roles = roles_in(&tokens, catalog);
    if roles.is_empty() {
        return None;
    }
    let give = has_prefix(&tokens, &GIVE_PREFIXES) || has_word(&tokens, &["daj"]);
    let remove = has_prefix(&tokens, &REMOVE_PREFIXES)
        || (has_word(&tokens, &["nie"]) && has_word(&tokens, &["juz"]));
    let name_words = mentions.iter().filter(|m| m.is_address_form()).count();
    let short_ty = has_word(&tokens, &["ty"]) && tokens.len() - name_words <= 3;
    let strong = has_prefix(&tokens, &STRONG_PREFIXES)
        || has_word(&tokens, &STRONG_WORDS)
        || has_seq(&tokens, &["teraz", "ty"])
        || has_seq(&tokens, &["od", "teraz"])
        || has_seq(&tokens, &["od", "dzis"])
        || has_seq(&tokens, &["od", "tej", "chwili"])
        || mentions
            .iter()
            .any(|m| m.index > 0 && tokens[m.index - 1].word == "teraz");
    if !(give || remove || strong || short_ty) {
        return None;
    }

    let target = if give {
        let receiver = mentions
            .iter()
            .find(|m| m.cases.contains(&Case::Dative) && Some(&m.persona) != addressee.as_ref())
            .map(|m| m.persona.clone());
        receiver
            .or_else(|| first_with(&mentions, &[Case::Dative, Case::Accusative]))
            .or_else(any_mention)
    } else {
        addressee.clone().or_else(|| {
            first_with(
                &mentions,
                &[Case::Nominative, Case::Vocative, Case::Accusative],
            )
        })
    }?;

    Some(if remove {
        CastCommand::Remove {
            persona: target,
            roles,
        }
    } else {
        CastCommand::Assign {
            persona: target,
            roles,
            exclusive: !has_word(&tokens, &ADDITIVE),
        }
    })
}

/// Stosuje polecenie do obsady i waliduje wynik (obsada wejściowa bez zmian przy błędzie).
pub fn apply_command(
    cast: &Cast,
    command: &CastCommand,
    catalog: &Catalog,
) -> Result<Cast, CastError> {
    let next = match command {
        CastCommand::Template { template, persona } => {
            catalog.cast_from_template(template, persona.as_ref(), cast.voice)?
        }
        CastCommand::Assign {
            persona,
            roles,
            exclusive,
        } => {
            let mut next = cast.clone();
            next.template = None;
            for role in roles {
                let unique = catalog.role(role).is_some_and(|r| r.unique);
                next.assign(persona, role, *exclusive || unique);
            }
            next
        }
        CastCommand::Remove { persona, roles } => {
            let mut next = cast.clone();
            next.template = None;
            for role in roles {
                next.unassign(persona, role);
            }
            next
        }
    };
    catalog.validate_cast(&next)?;
    Ok(next)
}
