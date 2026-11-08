//! Poprawne argumenty narzędzi: generator ze schematu (`input_schema`) i przepisy dla narzędzi,
//! którym sam schemat nie wystarcza (konkretny plik, adres, kolejność wywołań). Narzędzie dodane
//! później dostaje argumenty z generatora od razu — przepis dopisuje się tylko, gdy raport
//! pokaże `InvalidArgs` albo działanie na pustych danych.

use serde_json::{Map, Value, json};

use super::fakes::Targets;

/// Przykładowa wartość dla schematu; `name` — nazwa pola (podpowiedź dla ścieżek i adresów).
pub fn sample(schema: &Value, name: &str) -> Value {
    if let Some(v) = schema.get("const") {
        return v.clone();
    }
    if let Some(first) = schema["enum"].as_array().and_then(|e| e.first()) {
        return first.clone();
    }
    if let Some(v) = schema.get("default").filter(|v| !v.is_null()) {
        return v.clone();
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(alt) = schema[key]
            .as_array()
            .and_then(|a| a.iter().find(|s| s["type"] != "null"))
        {
            return sample(alt, name);
        }
    }
    match type_of(schema) {
        "object" => object(schema),
        "array" => {
            let n = schema["minItems"].as_u64().unwrap_or(1).max(1);
            let item = sample(&schema["items"], name);
            Value::Array((0..n).map(|_| item.clone()).collect())
        }
        "integer" => json!(number(schema).round() as i64),
        "number" => json!(number(schema)),
        "boolean" => json!(false),
        "null" => Value::Null,
        _ => json!(string(schema, name)),
    }
}

/// Typ ze schematu (`["string","null"]` → `string`; brak typu przy `properties` → `object`).
fn type_of(schema: &Value) -> &str {
    match &schema["type"] {
        Value::String(t) => t,
        Value::Array(types) => types
            .iter()
            .filter_map(Value::as_str)
            .find(|t| *t != "null")
            .unwrap_or("null"),
        _ if schema.get("properties").is_some() => "object",
        _ => "string",
    }
}

/// Obiekt z wymaganymi polami.
fn object(schema: &Value) -> Value {
    let mut out = Map::new();
    let required = schema["required"].as_array().cloned().unwrap_or_default();
    for key in required.iter().filter_map(Value::as_str) {
        out.insert(key.into(), sample(&schema["properties"][key], key));
    }
    Value::Object(out)
}

/// Liczba w granicach schematu (1, gdy granice na to pozwalają).
fn number(schema: &Value) -> f64 {
    let min = schema["minimum"]
        .as_f64()
        .or_else(|| schema["exclusiveMinimum"].as_f64().map(|m| m + 1.0));
    let max = schema["maximum"].as_f64();
    let n = min.map_or(1.0, |m| m.max(1.0));
    max.map_or(n, |m| n.min(m))
}

/// Tekst po nazwie pola: ścieżki wskazują pliki katalogu roboczego, adresy — atrapę sieci.
fn string(schema: &Value, name: &str) -> String {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    let text = if schema["format"] == "uri" || has(&["url", "href"]) {
        "https://example.com/"
    } else if has(&["dest", "target", "to", "new"]) {
        "kopia.txt"
    } else if has(&["dir", "folder", "cwd"]) {
        "."
    } else if has(&["path", "file", "source", "src", "from"]) {
        "notatki.txt"
    } else if has(&["command", "cmd"]) {
        "echo test"
    } else {
        "test"
    };
    let min = schema["minLength"].as_u64().unwrap_or(0) as usize;
    format!("{text}{}", "x".repeat(min.saturating_sub(text.len())))
}

/// Przepis: pola nadpisujące wynik generatora (konkretny plik, okno, element, proces).
/// `None` — wystarczy schemat.
pub fn recipe(tool: &str, t: &Targets) -> Option<Value> {
    let w = t.window;
    Some(match tool {
        "browser_click" => json!({ "node": 1 }),
        // Świeża karta nie ma historii — „wstecz” dałoby `not_found`; odświeżenie działa zawsze.
        "browser_nav" => json!({ "action": "reload" }),
        "browser_type" => json!({ "node": 2, "text": "diagnostyka" }),
        "browser_select" => json!({ "node": 3, "value": "Polska" }),
        "clipboard_write" => json!({ "text": "diagnostyka" }),
        "engine_start" => json!({ "window": true }),
        "engine_transaction" => json!({
            "operations": [{ "op": "entity.spawn", "entity": { "tag": "skrzynia" } }]
        }),
        "fs_copy" => json!({ "from": "do-kopii.txt", "to": "kopia.txt" }),
        "fs_delete" => json!({ "path": "do-usuniecia.txt" }),
        "fs_delete_permanent" => json!({ "path": "do-usuniecia-trwale.txt" }),
        "fs_list" => json!({ "path": "." }),
        "fs_move" => json!({ "from": "do-przeniesienia.txt", "to": "podkatalog/przeniesiony.txt" }),
        "fs_rename" => json!({ "path": "do-zmiany-nazwy.txt", "new_name": "nowa-nazwa.txt" }),
        "fs_search" => json!({ "pattern": "*.txt", "root": "." }),
        "browser_wait" => json!({ "text": "Strona", "timeout_ms": 1000 }),
        "system_env_set" => json!({ "name": "DIAG_ZMIENNA", "value": "1" }),
        "fs_write" => json!({ "path": "nowy.txt", "content": "diagnostyka" }),
        "input_click" => json!({ "window": w, "element": t.button }),
        "input_keys" => json!({ "window": w, "keys": ["ctrl+a"] }),
        "input_scroll" | "uia_tree" | "window_focus" => json!({ "window": w }),
        "input_type_text" => json!({ "window": w, "text": "diagnostyka" }),
        "fs_mkdir" => json!({ "path": "nowy-katalog" }),
        "media_convert" => json!({ "path": "dzwiek.wav", "format": "mp3" }),
        "media_info" | "media_play" => json!({ "path": "dzwiek.wav" }),
        "net_download" => json!({ "url": "https://example.com/plik.txt" }),
        "office_edit" | "office_read" => json!({ "path": "raport.docx" }),
        "screen_capture" => json!({ "target": "window", "window": w }),
        "system_process_info" => json!({ "pid": t.pid }),
        "system_process_kill" => json!({ "pid": t.pid, "name": "kalkulator.exe" }),
        "system_service_control" => json!({ "name": t.service }),
        "uia_act" => json!({ "element": t.button, "action": "invoke" }),
        "uia_find" => json!({ "window": w, "name": "Zapisz" }),
        "uia_read_text" => json!({ "element": t.edit }),
        "vision_describe" => json!({ "source": "file", "path": "obraz.png" }),
        "window_move" => json!({ "window": w, "x": 10, "y": 10, "width": 640, "height": 480 }),
        "window_state" => json!({ "window": w, "state": "maximized" }),
        _ => return None,
    })
}

/// Próby straży: wywołania, które MUSZĄ zostać odrzucone na każdym poziomie (także L4) —
/// (narzędzie, argumenty, oczekiwany początek klasy wyniku). `secret` — plik poświadczeń w profilu.
pub fn guard_probes(secret: &str) -> Vec<(&'static str, Value, &'static str)> {
    vec![
        (
            "system_env_set",
            json!({ "name": "ALFA_DIAG", "value": "1" }),
            "odmowa:policy",
        ),
        ("fs_read", json!({ "path": secret }), "odmowa"),
        ("fs_delete", json!({ "path": secret }), "odmowa"),
        // Strażnik sieci odrzuca adres niepubliczny jako błędny argument (bez połączenia).
        (
            "net_fetch",
            json!({ "url": "https://127.0.0.1/" }),
            "blad:invalid_args",
        ),
        (
            "shell_run",
            json!({ "command": format!("type \"{secret}\"") }),
            "odmowa",
        ),
    ]
}

/// Odmowy argumentów, które są poprawnym wynikiem atrapy (z powodem do raportu).
pub fn known_rejection(tool: &str) -> Option<&'static str> {
    match tool {
        "engine_height" => {
            Some("atrapa silnika bez terenu strumieniowanego (jak silnik bez terenu)")
        }
        _ => None,
    }
}

/// Dlaczego wynik narzędzia we wzorcu nie jest `ok` (do raportu).
pub fn note(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "browser_click" | "browser_key" | "browser_nav" | "browser_select" | "browser_type" => {
            "działanie na stronie po obcej treści wymaga zgody właściciela także na L4 (rdzeń \
             testowy nie ma okna Sędziego Cieni — prośba wygasa); wykonanie sprawdzają testy \
             tools-browser-impl"
        }
        "fs_delete_permanent" => "trwałe usunięcie zawsze czeka na potwierdzenie (raport 44)",
        "shell_terminal" => "terminal tylko proponuje polecenie — uruchamia je właściciel",
        "mcp_call" => "brak skonfigurowanych serwerów MCP w profilu testowym",
        _ => return known_rejection(tool),
    })
}

/// Kolejność wywołań: najpierw otwarcie przeglądarki i start silnika, na końcu zamknięcia.
pub fn phase(tool: &str) -> u8 {
    match tool {
        "browser_open" | "engine_start" => 0,
        "browser_close" | "engine_stop" | "engine_rollback" => 2,
        _ => 1,
    }
}

/// Argumenty narzędzia: wynik generatora z nadpisanymi polami przepisu.
pub fn args_for(tool: &str, schema: &Value, t: &Targets) -> (Value, &'static str) {
    let mut args = sample(schema, "");
    let Some(Value::Object(fields)) = recipe(tool, t) else {
        return (args, "schemat");
    };
    if let Value::Object(out) = &mut args {
        out.extend(fields);
    }
    (args, "przepis")
}
