use providers_contract::{ToolChoice, ToolSpec};
use serde_json::json;

use super::*;

fn spec(name: &str, schema: Value) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: format!("Narzędzie {name} do testów schematu."),
        input_schema: schema,
        strict: false,
    }
}

#[test]
fn parameters_are_closed_and_refs_inlined() {
    let s = spec(
        "mcp_zapis",
        json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Zapis",
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "opcje": {"$ref": "#/$defs/Opcje"},
                "properties": {"type": "object", "properties": {"x": {"type": "integer"}}},
                "wolne": {"type": "object"},
                "lista": {"type": "array", "items": {"$ref": "#/definitions/Wpis"}},
                "suma": {"allOf": [
                    {"type": "object", "properties": {"a": {"type": "integer"}}},
                    {"properties": {"b": {"type": "integer"}}}
                ]},
                "tryb": {"enum": [{"properties": {}}]},
                "petla": {"$ref": "#"}
            },
            "required": ["path"],
            "$defs": {"Opcje": {"type": "object", "properties": {"nadpisz": {"type": "boolean"}}}},
            "definitions": {"Wpis": {"type": "object", "properties": {"k": {"type": "string"}},
                                     "additionalProperties": true}}
        }),
    );
    let p = tool_parameters(&s);
    for key in ["$schema", "title", "$defs", "definitions"] {
        assert!(p.get(key).is_none(), "{key} usunięte z korzenia");
    }
    assert_eq!(p["additionalProperties"], json!(false));
    let props = &p["properties"];
    assert_eq!(props["opcje"]["type"], "object", "$ref rozwinięte");
    assert_eq!(props["opcje"]["additionalProperties"], json!(false));
    assert_eq!(
        props["properties"]["additionalProperties"],
        json!(false),
        "pole o nazwie `properties` to schemat, nie mapa"
    );
    assert!(
        props.get("additionalProperties").is_none(),
        "mapa pól bez wpisu"
    );
    assert!(
        props["wolne"].get("additionalProperties").is_none(),
        "obiekt bez pól zostaje otwarty"
    );
    assert_eq!(
        props["lista"]["items"]["additionalProperties"],
        json!(true),
        "jawne additionalProperties zostaje"
    );
    let parts = props["suma"]["allOf"].as_array().unwrap();
    assert!(
        parts
            .iter()
            .all(|p| p.get("additionalProperties").is_none())
    );
    assert_eq!(
        props["tryb"]["enum"],
        json!([{"properties": {}}]),
        "wartości bez zmian"
    );
    // Odwołanie do korzenia (rekurencja) rozwija się do limitu głębokości, bez zawieszenia.
    assert!(props["petla"].is_object());
    assert!(jsonschema::validator_for(&p).is_ok());
}

#[test]
fn missing_or_non_object_schema_means_any_object() {
    for schema in [Value::Null, json!("x"), json!(true)] {
        assert_eq!(
            tool_parameters(&spec("t", schema)),
            json!({"type": "object"})
        );
    }
}

#[test]
fn envelope_has_text_and_an_optional_named_call() {
    let tools = [
        spec(
            "zeta",
            json!({"type": "object", "properties": {"n": {"type": "integer"}}}),
        ),
        spec(
            "alfa",
            json!({"type": "object", "properties": {"s": {"type": "string"}}}),
        ),
    ];
    let offered = offered(&tools, &ToolChoice::Auto);
    assert_eq!(offered[0].name, "alfa", "kolejność nazw");
    let schema = envelope(&offered, false);
    assert_eq!(schema["required"], json!(["response"]));
    assert_eq!(schema["additionalProperties"], json!(false));
    let keys: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["response", "tool_call"], "tekst przed wywołaniem");
    let calls = schema["properties"]["tool_call"]["anyOf"]
        .as_array()
        .unwrap();
    assert_eq!(calls[0]["properties"]["name"], json!({"const": "alfa"}));
    let call_keys: Vec<&String> = calls[0]["properties"].as_object().unwrap().keys().collect();
    assert_eq!(call_keys, ["name", "parameters"], "nazwa przed argumentami");
    assert_eq!(
        calls[1]["properties"]["parameters"]["additionalProperties"],
        json!(false)
    );
    let validator = jsonschema::validator_for(&schema).unwrap();
    let call = |name: &str, params: Value| json!({"name": name, "parameters": params});
    assert!(validator.is_valid(&json!({"response": "Cześć"})));
    assert!(
        validator.is_valid(
            &json!({"response": "Sprawdzam.", "tool_call": call("zeta", json!({"n": 1}))})
        )
    );
    assert!(
        !validator.is_valid(&json!({"response": "", "tool_call": call("zeta", json!({"n": "1"}))}))
    );
    assert!(!validator.is_valid(&json!({"response": "", "tool_call": call("inne", json!({}))})));
    assert!(
        !validator.is_valid(&json!({"tool_call": call("zeta", json!({"n": 1}))})),
        "bez tekstu"
    );
    assert!(!validator.is_valid(&json!({"response": "x", "extra": 1})));
    let format = response_format(schema);
    assert_eq!(format["type"], "json_schema");
    assert_eq!(
        format["json_schema"]["schema"]["required"],
        json!(["response"])
    );
}

#[test]
fn forced_choice_requires_the_call() {
    let tools = [
        spec("a", json!({"type": "object"})),
        spec("b", json!({"type": "object"})),
    ];
    let named = ToolChoice::Tool { name: "b".into() };
    let only = offered(&tools, &named);
    assert_eq!(only.len(), 1);
    assert!(must_call(&named) && must_call(&ToolChoice::Required));
    assert!(!must_call(&ToolChoice::Auto));
    let schema = envelope(&only, true);
    assert_eq!(schema["required"], json!(["response", "tool_call"]));
    assert_eq!(
        schema["properties"]["tool_call"]["properties"]["name"],
        json!({"const": "b"})
    );
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(!validator.is_valid(&json!({"response": "Bez wywołania."})));
    let unknown = ToolChoice::Tool { name: "zz".into() };
    assert_eq!(
        offered(&tools, &unknown).len(),
        2,
        "nieznane narzędzie → wszystkie"
    );
}

#[test]
fn argument_errors_are_short_and_located() {
    let schema = tool_parameters(&spec(
        "t",
        json!({"type": "object", "properties": {"path": {"type": "string"}, "n": {"type": "integer"}},
               "required": ["path"]}),
    ));
    assert!(check_arguments(&schema, &json!({"path": "C:/a"})).is_ok());
    let err = check_arguments(&schema, &json!({"n": "x"})).unwrap_err();
    assert!(err.contains("path") && err.contains("/n"), "{err}");
    let err = check_arguments(&schema, &json!({"path": "a", "rm": true})).unwrap_err();
    assert!(err.contains("rm"), "{err}");
    let long = json!({"path": "x".repeat(2_000), "n": "y".repeat(2_000)});
    let tight = json!({"type": "object", "properties": {"path": {"maxLength": 1}, "n": {"type": "integer"}}});
    let err = check_arguments(&tight, &long).unwrap_err();
    assert!(err.chars().count() <= MAX_ERROR_CHARS + 1, "{}", err.len());
    // Schemat, którego walidator nie zbuduje, nie blokuje wywołania.
    assert!(check_arguments(&json!({"type": 12}), &json!({})).is_ok());
}
