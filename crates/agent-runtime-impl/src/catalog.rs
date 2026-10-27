//! Lokalny indeks dostępnych narzędzi: krótka komenda, nazwa, kategoria i opis PL/EN.
use providers_contract::ToolSpec;
use serde_json::json;
use std::collections::BTreeSet;
use tools_common_contract::ToolManifest;

pub(crate) fn select_specs(
    available: Vec<&ToolManifest>,
    catalog: &Catalog,
    goal: &str,
    discovered: &[String],
    read_only: bool,
) -> Vec<ToolSpec> {
    if available.len() <= VISIBLE_LIMIT {
        return available.into_iter().map(ToolManifest::to_spec).collect();
    }
    let allowed: BTreeSet<_> = available.iter().map(|m| m.name.as_str()).collect();
    let mut selected = BTreeSet::new();
    for name in CORE
        .iter()
        .copied()
        .map(str::to_owned)
        .chain(discovered.iter().rev().cloned())
        .chain(catalog.rank(goal, read_only, SEARCH_LIMIT))
    {
        if allowed.contains(name.as_str()) && selected.len() < VISIBLE_LIMIT - 2 {
            selected.insert(name);
        }
    }
    let mut specs: Vec<_> = available
        .into_iter()
        .filter(|m| selected.contains(&m.name))
        .map(ToolManifest::to_spec)
        .collect();
    specs.extend(builtin_specs());
    specs
}

pub(crate) const FIND: &str = "tools_find";
pub(crate) const CALL: &str = "tool_call";
pub(crate) const VISIBLE_LIMIT: usize = 24;
pub(crate) const SEARCH_LIMIT: usize = 8;
pub(crate) const CORE: &[&str] = &[
    "fs_list",
    "fs_read",
    "fs_search",
    "fs_stat",
    "fs_write",
    "shell_run",
    "delegate_task",
];
pub(crate) const QUICK_GUIDE: &str = "\nSzybki dostęp do narzędzi: otrzymujesz pakiet dobrany do celu, a nie cały katalog. Jeśli znasz operację i argumenty, wywołaj ją od razu (albo tool_call z krótką command i args). tools_find służy tylko do znalezienia brakującej definicji; nie ponawiaj wyszukiwania znanej operacji. Znana ścieżka → read/fs_read; nieznana lokalizacja → find/fs_search. Plik edytuj przez write/edit/fs_write, terminal zostaw do testów i poleceń. Nie przeglądaj zainstalowanych programów ani całego dysku, jeżeli zadanie tego nie wymaga. Każde wywołanie nadal podlega profilowi dostępu i Brokerowi.\n";

pub(crate) fn alias(command: &str) -> &str {
    match command {
        "read" | "czytaj" => "fs_read",
        "list" | "lista" => "fs_list",
        "find" | "szukaj" => "fs_search",
        "stat" => "fs_stat",
        "edit" | "write" | "zapisz" => "fs_write",
        "run" | "uruchom" => "shell_run",
        "fetch" | "pobierz" => "net_fetch",
        "tools" | "narzedzia" | "narzędzia" => FIND,
        other => other,
    }
}

fn normal(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect()
}

#[derive(Clone)]
struct Entry {
    name: String,
    normalized_name: String,
    title: String,
    description: String,
    group: String,
    mutating: bool,
}
#[derive(Clone, Default)]
pub(crate) struct Catalog {
    entries: Vec<Entry>,
}

impl Catalog {
    pub fn new<'a>(manifests: impl Iterator<Item = &'a ToolManifest>) -> Self {
        Self {
            entries: manifests
                .map(|m| Entry {
                    name: m.name.clone(),
                    normalized_name: normal(&m.name),
                    title: normal(&m.title),
                    description: normal(&m.description),
                    group: normal(&m.groups.join(" ")),
                    mutating: m.mutating,
                })
                .collect(),
        }
    }
    pub fn rank(&self, query: &str, read_only: bool, limit: usize) -> Vec<String> {
        if limit == 0 {
            return Vec::new();
        }
        let command = query.trim().to_lowercase();
        let canonical = alias(&command);
        if let Some(e) = self
            .entries
            .iter()
            .find(|e| (!read_only || !e.mutating) && e.name.eq_ignore_ascii_case(canonical))
        {
            return vec![e.name.clone()];
        }
        let query = normal(query);
        let words: Vec<_> = query.split_whitespace().filter(|w| w.len() > 1).collect();
        let domains: &[(&[&str], &str)] = &[
            (
                &[
                    "plik",
                    "folder",
                    "repo",
                    "kod",
                    "file",
                    "read",
                    "write",
                    "directory",
                ],
                "fs",
            ),
            (
                &[
                    "terminal", "polecen", "uruchom", "shell", "command", "test", "build",
                ],
                "shell",
            ),
            (
                &["silnik", "nextgo", "engine", "game", "shader", "raycast"],
                "engine",
            ),
            (&["stron", "przeglad", "browser", "website"], "browser"),
            (
                &["http", "pobierz", "fetch", "download", "internet", "siec"],
                "net",
            ),
            (&["pamiec", "zapamiet", "memory", "remember"], "memory"),
            (
                &["dokument", "arkusz", "excel", "word", "office", "document"],
                "office",
            ),
            (
                &["mysz", "klawiatur", "ekran", "mouse", "keyboard", "screen"],
                "gui",
            ),
            (&["kamera", "obraz", "camera", "image", "ocr"], "vision"),
            (
                &["system", "proces", "uslug", "process", "service"],
                "system",
            ),
        ];
        let mut found: Vec<_> = self
            .entries
            .iter()
            .filter(|e| !read_only || !e.mutating)
            .map(|e| {
                let name = &e.normalized_name;
                let exact = i32::from(e.name.eq_ignore_ascii_case(canonical)) * 10_000;
                let typo = i32::from(near_name(canonical, &e.name)) * 300;
                let words_score: i32 = words
                    .iter()
                    .map(|w| {
                        i32::from(name.contains(w)) * 80
                            + i32::from(e.title.contains(w)) * 25
                            + i32::from(e.group.contains(w)) * 30
                            + i32::from(e.description.contains(w)) * 3
                    })
                    .sum();
                let domain_score: i32 = domains
                    .iter()
                    .filter(|(hints, group)| {
                        hints.iter().any(|h| query.contains(h))
                            && e.group.split_whitespace().any(|g| g == *group)
                    })
                    .map(|_| 45)
                    .sum();
                (exact + typo + words_score + domain_score, &e.name)
            })
            .filter(|(score, _)| *score > 0)
            .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        found
            .into_iter()
            .take(limit)
            .map(|(_, name)| name.clone())
            .collect()
    }
}

/// Jedna literówka albo przestawienie sąsiednich liter w krótkiej nazwie komendy.
fn near_name(a: &str, b: &str) -> bool {
    if !a.is_ascii()
        || !b.is_ascii()
        || a.len() < 4
        || a.len() > 64
        || a.contains(' ')
        || a.len().abs_diff(b.len()) > 1
    {
        return false;
    }
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() == b.len() {
        let diff: Vec<_> = a
            .iter()
            .zip(b)
            .enumerate()
            .filter(|(_, (x, y))| x != y)
            .map(|(i, _)| i)
            .take(3)
            .collect();
        return match diff.as_slice() {
            [_] => true,
            [i, j] => *j == *i + 1 && a[*i] == b[*j] && a[*j] == b[*i],
            _ => false,
        };
    }
    let (short, long) = if a.len() < b.len() { (a, b) } else { (b, a) };
    let mut i = 0;
    while i < short.len() && short[i] == long[i] {
        i += 1;
    }
    short[i..] == long[i + 1..]
}

pub(crate) fn builtin_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec { name:FIND.into(), description: "Znajdź dostępne narzędzia po komendzie (read, edit, run), nazwie, kategorii lub opisie PL/EN. Najwięcej 8 wyników; ich pełne schematy będą dostępne w następnej turze. Nie przeszukuje plików ani internetu.".into(),
            input_schema:json!({"type":"object","properties":{"query":{"type":"string","minLength":1,"maxLength":512}},"required":["query"],"additionalProperties":false}), strict:false },
        ToolSpec { name:CALL.into(), description:"Krótka komenda do dostępnego narzędzia. read/list/find/stat = operacje plikowe; write/edit = fs_write (wymaga content); run = shell_run; fetch = net_fetch. args musi odpowiadać schematowi narzędzia. Jeśli go nie znasz, tools_find(query). To nie jest interpreter kodu.".into(),
            input_schema:json!({"type":"object","properties":{"command":{"type":"string","maxLength":64},"args":{"type":"object","additionalProperties":true}},"required":["command","args"],"additionalProperties":false}), strict:false },
    ]
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
