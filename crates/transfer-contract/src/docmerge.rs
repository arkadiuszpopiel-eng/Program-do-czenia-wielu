//! Dokumenty przy imporcie: format po rozszerzeniu, scalanie (TOML/JSON klucz po kluczu — paczka
//! nadpisuje; NDJSON — suma wpisów z deduplikacją po `id` i treści) oraz usuwanie kluczy polityk
//! Jądra (`kernel.*` zmienia wyłącznie Broker — import ich nie zapisuje).

use std::collections::BTreeSet;

use crate::error::TransferError;

/// Prefiks kluczy polityk Jądra (`core-config-contract::KERNEL_POLICY_PREFIX`).
pub const KERNEL_PREFIX: &str = "kernel";

/// Format dokumentu (po rozszerzeniu nazwy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocFormat {
    /// `.toml`
    Toml,
    /// `.json`
    Json,
    /// `.ndjson`
    Ndjson,
    /// Inne — nie da się scalić.
    Opaque,
}

/// Format po rozszerzeniu.
pub fn format_of(name: &str) -> DocFormat {
    match name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()) {
        Some(e) if e == "toml" => DocFormat::Toml,
        Some(e) if e == "json" => DocFormat::Json,
        Some(e) if e == "ndjson" => DocFormat::Ndjson,
        _ => DocFormat::Opaque,
    }
}

fn parse_toml(name: &str, bytes: &[u8]) -> Result<toml::Table, TransferError> {
    let text = std::str::from_utf8(bytes).map_err(|e| TransferError::invalid(name, e))?;
    toml::from_str(text).map_err(|e| TransferError::invalid(name, e))
}

fn to_toml(name: &str, table: &toml::Table) -> Result<Vec<u8>, TransferError> {
    toml::to_string(table)
        .map(String::into_bytes)
        .map_err(|e| TransferError::invalid(name, e))
}

/// Usuwa klucze `kernel.*` z konfiguracji TOML (także w tabelach zakresów `"@session".<id>`,
/// `"@agent".<id>`). Zwraca treść (bez zmian, gdy nic nie usunięto) i listę usuniętych kluczy.
pub fn strip_kernel_keys(
    name: &str,
    bytes: &[u8],
) -> Result<(Vec<u8>, Vec<String>), TransferError> {
    let mut table = parse_toml(name, bytes)?;
    let mut removed = Vec::new();
    strip_in(&mut table, "", &mut removed);
    if removed.is_empty() {
        return Ok((bytes.to_vec(), removed));
    }
    Ok((to_toml(name, &table)?, removed))
}

fn strip_in(table: &mut toml::Table, prefix: &str, removed: &mut Vec<String>) {
    if let Some(kernel) = table.remove(KERNEL_PREFIX) {
        collect_keys(&kernel, &format!("{prefix}{KERNEL_PREFIX}"), removed);
    }
    for (key, value) in table.iter_mut() {
        if key.starts_with('@')
            && let toml::Value::Table(scopes) = value
        {
            for (id, scoped) in scopes.iter_mut() {
                if let toml::Value::Table(inner) = scoped {
                    strip_in(inner, &format!("{key}.{id}."), removed);
                }
            }
        }
    }
}

fn collect_keys(value: &toml::Value, path: &str, out: &mut Vec<String>) {
    match value {
        toml::Value::Table(t) => t
            .iter()
            .for_each(|(k, v)| collect_keys(v, &format!("{path}.{k}"), out)),
        _ => out.push(path.to_owned()),
    }
}

/// Scala dokument lokalny z dokumentem z paczki. `None` — formatu nie da się scalić.
pub fn merge_documents(
    name: &str,
    local: &[u8],
    package: &[u8],
) -> Result<Option<Vec<u8>>, TransferError> {
    match format_of(name) {
        DocFormat::Toml => {
            let mut base = toml::Value::Table(parse_toml(name, local)?);
            merge_toml(&mut base, toml::Value::Table(parse_toml(name, package)?));
            match base {
                toml::Value::Table(t) => to_toml(name, &t).map(Some),
                _ => Ok(None),
            }
        }
        DocFormat::Json => {
            let mut base: serde_json::Value =
                serde_json::from_slice(local).map_err(|e| TransferError::invalid(name, e))?;
            let over: serde_json::Value =
                serde_json::from_slice(package).map_err(|e| TransferError::invalid(name, e))?;
            merge_json(&mut base, over);
            serde_json::to_vec_pretty(&base)
                .map(Some)
                .map_err(|e| TransferError::invalid(name, e))
        }
        DocFormat::Ndjson => merge_ndjson(name, local, package).map(Some),
        DocFormat::Opaque => Ok(None),
    }
}

/// Scalanie TOML: tabele rekurencyjnie, pozostałe wartości (w tym tablice) — paczka wygrywa.
fn merge_toml(base: &mut toml::Value, over: toml::Value) {
    match (base, over) {
        (toml::Value::Table(b), toml::Value::Table(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(existing) => merge_toml(existing, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (b, o) => *b = o,
    }
}

/// Scalanie JSON: obiekty rekurencyjnie, pozostałe wartości — paczka wygrywa.
fn merge_json(base: &mut serde_json::Value, over: serde_json::Value) {
    match (base, over) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(existing) => merge_json(existing, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (b, o) => *b = o,
    }
}

/// Suma wpisów NDJSON: lokalne w kolejności, potem nowe z paczki; duplikat = ten sam `id`
/// (gdy wpis go ma) albo ta sama treść.
fn merge_ndjson(name: &str, local: &[u8], package: &[u8]) -> Result<Vec<u8>, TransferError> {
    let text = |b: &[u8]| -> Result<String, TransferError> {
        String::from_utf8(b.to_vec()).map_err(|e| TransferError::invalid(name, e))
    };
    let (local, package) = (text(local)?, text(package)?);
    let key = |line: &str| -> Result<String, TransferError> {
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| TransferError::invalid(name, e))?;
        Ok(match v.get("id") {
            Some(id) => format!("id:{id}"),
            None => format!("line:{}", crate::manifest::sha256_hex(line.as_bytes())),
        })
    };
    let mut seen = BTreeSet::new();
    let mut out = String::with_capacity(local.len() + package.len());
    for line in local.lines().chain(package.lines()) {
        if line.trim().is_empty() {
            continue;
        }
        let k = key(line)?;
        let content = format!("line:{}", crate::manifest::sha256_hex(line.as_bytes()));
        if seen.contains(&k) || seen.contains(&content) {
            continue;
        }
        seen.insert(k);
        seen.insert(content);
        out.push_str(line);
        out.push('\n');
    }
    Ok(out.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(format_of("shared.toml"), DocFormat::Toml);
        assert_eq!(format_of("p.JSON"), DocFormat::Json);
        assert_eq!(format_of("m.ndjson"), DocFormat::Ndjson);
        assert_eq!(format_of("a.bin"), DocFormat::Opaque);
        assert_eq!(format_of("bez"), DocFormat::Opaque);
    }

    #[test]
    fn kernel_keys_are_stripped_everywhere() {
        let doc = b"[kernel.egress]\nallow = [\"x\"]\n[voice]\nengine = \"piper\"\n[\"@session\".s1.kernel]\nbudget = 5\n[\"@session\".s1.ui]\ntheme = \"dark\"\n";
        let (out, removed) = strip_kernel_keys("shared.toml", doc).unwrap();
        assert_eq!(
            removed,
            vec!["kernel.egress.allow", "@session.s1.kernel.budget"]
        );
        let t: toml::Table = toml::from_str(std::str::from_utf8(&out).unwrap()).unwrap();
        assert!(t.get("kernel").is_none());
        assert_eq!(t["voice"]["engine"].as_str(), Some("piper"));
        assert_eq!(t["@session"]["s1"]["ui"]["theme"].as_str(), Some("dark"));
        let plain = b"a = 1\n";
        assert_eq!(
            strip_kernel_keys("x.toml", plain).unwrap(),
            (plain.to_vec(), vec![])
        );
        assert!(strip_kernel_keys("x.toml", b"a = ").is_err());
    }

    #[test]
    fn merges_key_by_key_package_wins() {
        let local = b"[voice]\nengine = \"piper\"\nrate = 1\n[ui]\ntheme = \"light\"\n";
        let pkg = b"[voice]\nengine = \"pocket\"\n[new]\nx = true\n";
        let out = merge_documents("s.toml", local, pkg).unwrap().unwrap();
        let t: toml::Table = toml::from_str(std::str::from_utf8(&out).unwrap()).unwrap();
        assert_eq!(t["voice"]["engine"].as_str(), Some("pocket"));
        assert_eq!(t["voice"]["rate"].as_integer(), Some(1));
        assert_eq!(t["ui"]["theme"].as_str(), Some("light"));
        assert_eq!(t["new"]["x"].as_bool(), Some(true));

        let out = merge_documents(
            "p.json",
            br#"{"a":{"b":1,"c":2},"l":[1]}"#,
            br#"{"a":{"b":9},"l":[2]}"#,
        )
        .unwrap()
        .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v, serde_json::json!({"a":{"b":9,"c":2},"l":[2]}));

        let out = merge_documents(
            "m.ndjson",
            b"{\"id\":1,\"t\":\"a\"}\n{\"t\":\"bez id\"}\n",
            b"{\"id\":1,\"t\":\"inna\"}\n{\"id\":2,\"t\":\"b\"}\n{\"t\":\"bez id\"}\n",
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            std::str::from_utf8(&out).unwrap(),
            "{\"id\":1,\"t\":\"a\"}\n{\"t\":\"bez id\"}\n{\"id\":2,\"t\":\"b\"}\n"
        );
        assert_eq!(merge_documents("a.bin", b"x", b"y").unwrap(), None);
    }
}
