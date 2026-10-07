//! Walidacja manifestu (pola, limity, zdolności ⊆ dozwolone, zakresy nie za szerokie,
//! narzędzia zgodne z `tools-common`, opisy bez znaczników wstrzyknięć), hashe (moduł Wasm,
//! kanoniczny manifest = to, co zatwierdza właściciel) i rozpoznanie nagłówka komponentu.

use std::collections::BTreeSet;

use compliance_contract::deny::Root;
use safety_broker_contract::{Capability, PathScope, hex};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::PluginError;
use crate::manifest::{
    ALLOWED_FAMILIES, FORBIDDEN_FAMILIES, MAX_WASM_BYTES, PluginManifest, PluginToolDecl,
};

/// Najwięcej zadeklarowanych zdolności.
pub const MAX_CAPABILITIES: usize = 16;
/// Najwięcej narzędzi.
pub const MAX_TOOLS: usize = 32;
/// Największy schemat narzędzia (B, kanoniczny JSON).
pub const MAX_SCHEMA_BYTES: usize = 16 * 1024;
/// Nagłówek binarny komponentu Wasm (magia + wersja + warstwa 1).
pub const COMPONENT_HEADER: [u8; 8] = [0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00];

/// Fragmenty (po złożeniu) wskazujące na próbę wstrzyknięcia w opisie dla modelu.
const SUSPICIOUS: [&str; 15] = [
    "zignoruj",
    "ignore previous",
    "ignore all",
    "disregard",
    "system prompt",
    "jailbreak",
    "<<<",
    ">>>",
    "broker",
    "kill-switch",
    "poziom autonomii",
    "~/.claude",
    ".codex",
    "secrets.read",
    "system.admin",
];

/// SHA-256 bajtów (hex, małe litery).
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(&Sha256::digest(bytes))
}

/// Czy tekst to hash SHA-256 w postaci kanonicznej (64 znaki hex, małe litery).
pub fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64 && hex::decode(text).is_some()
}

fn write_canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                if let Some(x) = m.get(k) {
                    write_canonical(x, out);
                }
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(x, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// Kanoniczny JSON (klucze posortowane, bez białych znaków — niezależny od kolejności).
pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, String> {
    let v = serde_json::to_value(value).map_err(|e| e.to_string())?;
    let mut out = String::new();
    write_canonical(&v, &mut out);
    Ok(out)
}

/// Hash przejrzanej wersji: SHA-256 kanonicznego manifestu (obejmuje `wasm_sha256`, więc
/// zatwierdzenie wiąże dokładne bajty modułu, zdolności, limity i opisy narzędzi).
pub fn review_hash(manifest: &PluginManifest) -> Result<String, PluginError> {
    canonical_json(manifest)
        .map(|c| sha256_hex(c.as_bytes()))
        .map_err(PluginError::Invalid)
}

/// Integralność zainstalowanej wersji: hash kanonicznego manifestu = hash rekordu = hash
/// zatwierdzony przez właściciela (kanał UI). Chroni przed podmianą rekordu w magazynie.
pub fn check_approved(record: &crate::model::PluginRecord) -> Result<(), crate::error::LoadError> {
    let computed = review_hash(&record.manifest).ok();
    let approved = record.approval.as_ref().filter(|a| {
        a.origin == crate::model::ApprovalOrigin::Ui && a.reviewed_hash == record.review_hash
    });
    if approved.is_some() && computed.as_deref() == Some(record.review_hash.as_str()) {
        Ok(())
    } else {
        Err(crate::error::LoadError::NotApproved)
    }
}

/// Czy bajty mają nagłówek komponentu (moduł rdzeniowy — np. z importami WASI — nie ma).
pub fn is_component(bytes: &[u8]) -> bool {
    bytes.starts_with(&COMPONENT_HEADER)
}

/// Tekst złożony do porównań: małe litery, bez polskich znaków i znaków niewidocznych,
/// zwinięte białe znaki.
pub fn fold(text: &str) -> String {
    let mapped: String = text
        .chars()
        .filter(|c| !is_hidden(*c))
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
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_hidden(c: char) -> bool {
    matches!(c, '\u{00AD}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FEFF}')
}

fn len_ok(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.trim().chars().count())
}

fn invalid(what: impl Into<String>) -> PluginError {
    PluginError::Invalid(what.into())
}

/// Czy tekst dla modelu zawiera znaczniki wstrzyknięcia.
pub fn suspicious(text: &str) -> Option<&'static str> {
    let folded = fold(text);
    SUSPICIOUS.iter().copied().find(|s| folded.contains(s))
}

fn check_scope(cap: &Capability, scope: &PathScope) -> Result<(), PluginError> {
    let norm = scope.norm();
    if !matches!(norm.root, Root::Drive(_)) {
        return Err(invalid(format!(
            "zakres `{cap}` musi być ścieżką na dysku lokalnym (bez udziałów sieciowych)"
        )));
    }
    let min = if scope.subtree() { 3 } else { 1 };
    if norm.comps.len() < min {
        return Err(invalid(format!(
            "zakres `{cap}` jest za szeroki (poddrzewo wymaga ≥ 3 składników ścieżki)"
        )));
    }
    Ok(())
}

/// Zdolności: ≤ 16, bez powtórzeń, rodziny ⊆ [`ALLOWED_FAMILIES`], zakresy nie za szerokie.
pub fn check_capabilities(caps: &[Capability]) -> Result<(), PluginError> {
    if caps.len() > MAX_CAPABILITIES {
        return Err(invalid("za dużo zdolności (≤ 16)"));
    }
    let mut seen = BTreeSet::new();
    for cap in caps {
        let family = cap.family();
        if FORBIDDEN_FAMILIES.contains(&family) || !ALLOWED_FAMILIES.contains(&family) {
            return Err(PluginError::ForbiddenCapability(family.to_owned()));
        }
        if !seen.insert(cap.to_string()) {
            return Err(invalid(format!("zdolność `{cap}` powtórzona")));
        }
        if let Some(scope) = cap.path_scope() {
            check_scope(cap, scope)?;
        }
    }
    Ok(())
}

fn valid_tool_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name.len() <= 40
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn check_tool(m: &PluginManifest, t: &PluginToolDecl) -> Result<(), PluginError> {
    if !valid_tool_name(&t.name) {
        return Err(invalid(format!(
            "nazwa narzędzia `{}` spoza [a-z][a-z0-9_]{{0,39}}",
            t.name
        )));
    }
    if !len_ok(&t.title, 1, 80) || !len_ok(&t.description, 20, 1000) {
        return Err(invalid(format!(
            "narzędzie `{}`: tytuł (1–80) albo opis (20–1000)",
            t.name
        )));
    }
    for text in [&t.title, &t.description] {
        if let Some(hit) = suspicious(text) {
            return Err(invalid(format!(
                "narzędzie `{}`: podejrzany opis („{hit}”)",
                t.name
            )));
        }
    }
    for schema in [&t.input_schema, &t.output_schema] {
        let size = canonical_json(schema)
            .map(|c| c.len())
            .unwrap_or(usize::MAX);
        if size > MAX_SCHEMA_BYTES {
            return Err(invalid(format!("narzędzie `{}`: schemat za duży", t.name)));
        }
    }
    m.tool_manifest(t)
        .validate()
        .map_err(|e| invalid(e.to_string()))
}

/// Pełna walidacja manifestu.
pub fn validate_manifest(m: &PluginManifest) -> Result<(), PluginError> {
    if !m.id.is_valid() {
        return Err(invalid("identyfikator spoza [a-z][a-z0-9-]{1,47}"));
    }
    if !len_ok(&m.author, 1, 80) || m.author.contains('\n') {
        return Err(invalid("autor (1–80 znaków, jedna linia)"));
    }
    if !len_ok(&m.description, 10, 1000) {
        return Err(invalid("opis (10–1000 znaków)"));
    }
    if !is_sha256_hex(&m.wasm_sha256) {
        return Err(invalid(
            "`wasm_sha256` musi być SHA-256 (64 znaki hex, małe litery)",
        ));
    }
    check_capabilities(&m.capabilities)?;
    if !m.limits.within_ceilings() {
        return Err(invalid(
            "limity poza zakresem (dodatnie i ≤ sufitów piaskownicy)",
        ));
    }
    if m.tools.is_empty() || m.tools.len() > MAX_TOOLS {
        return Err(invalid("narzędzia: 1–32"));
    }
    let mut names = BTreeSet::new();
    for t in &m.tools {
        if !names.insert(t.name.as_str()) {
            return Err(invalid(format!("narzędzie `{}` powtórzone", t.name)));
        }
        check_tool(m, t)?;
    }
    Ok(())
}

/// Wstępna kontrola bajtów modułu: rozmiar, nagłówek komponentu, zgodność z hashem manifestu.
pub fn check_wasm(m: &PluginManifest, wasm: &[u8]) -> Result<(), PluginError> {
    use crate::error::LoadError;
    if wasm.len() > MAX_WASM_BYTES {
        return Err(PluginError::Load(LoadError::TooLarge(wasm.len())));
    }
    let actual = sha256_hex(wasm);
    if actual != m.wasm_sha256 {
        return Err(PluginError::Load(LoadError::HashMismatch {
            expected: m.wasm_sha256.clone(),
            actual,
        }));
    }
    if !is_component(wasm) {
        return Err(PluginError::Load(LoadError::NotComponent));
    }
    Ok(())
}
