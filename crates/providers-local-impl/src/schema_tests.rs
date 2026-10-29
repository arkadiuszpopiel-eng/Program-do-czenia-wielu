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
    // Odwołanie do korzenia to rekurencja: dowolna wartość, bez rozwijania.
    assert_eq!(props["petla"], json!({}));
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
fn envelope_offers_a_call_first_or_a_plain_answer() {
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
    let schema = envelope(&parameters_of(&offered), false);
    // Wariant z wywołaniem: wymagane tylko `tool_call` — gramatyka stawia je przed tekstem.
    let calling = &schema["anyOf"][0];
    assert_eq!(
        calling["required"],
        json!(["tool_call"]),
        "wywołanie pierwsze"
    );
    assert_eq!(calling["additionalProperties"], json!(false));
    let plain = &schema["anyOf"][1];
    assert_eq!(plain["required"], json!(["response"]));
    let keys: Vec<&String> = plain["properties"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["response"], "odpowiedź bez wywołania");
    let calls = calling["properties"]["tool_call"]["anyOf"]
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
        validator.is_valid(&json!({"tool_call": call("zeta", json!({"n": 1}))})),
        "wywołanie bez tekstu"
    );
    assert!(!validator.is_valid(&json!({})), "pusty obiekt");
    assert!(!validator.is_valid(&json!({"response": "x", "extra": 1})));
    let format = response_format(schema);
    assert_eq!(format["type"], "json_schema");
    assert_eq!(
        format["json_schema"]["schema"]["anyOf"][1]["required"],
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
    let schema = envelope(&parameters_of(&only), true);
    assert_eq!(schema["required"], json!(["tool_call"]));
    assert!(
        schema.get("anyOf").is_none(),
        "bez wariantu z samą odpowiedzią"
    );
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

fn nodes(v: &Value) -> usize {
    match v {
        Value::Object(obj) => 1 + obj.values().map(nodes).sum::<usize>(),
        Value::Array(items) => 1 + items.iter().map(nodes).sum::<usize>(),
        _ => 1,
    }
}

#[test]
fn branching_recursion_stays_small() {
    // Wartość w stylu JSON: definicja odwołuje się do siebie wielokrotnie (rozgałęzienie 6).
    let refs: Vec<Value> = (0..6)
        .map(|i| json!({"type": "array", "items": {"$ref": "#/$defs/W"}, "minItems": i}))
        .collect();
    let s = spec(
        "mcp_drzewo",
        json!({
            "type": "object",
            "properties": {"w": {"$ref": "#/$defs/W"}, "x": {"$ref": "#/$defs/W"}},
            "$defs": {"W": {"anyOf": refs}}
        }),
    );
    let p = tool_parameters(&s);
    assert!(nodes(&p) < 1_000, "{} węzłów", nodes(&p));
    let first = &p["properties"]["w"]["anyOf"][0];
    assert_eq!(
        first["items"],
        json!({}),
        "rekurencja zatrzymana na pierwszym powtórzeniu"
    );
    // Łańcuch niezależnych definicji z rozgałęzieniem też ma twardy limit (budżet węzłów).
    let mut defs = Map::new();
    for i in 0..30 {
        let next = format!("#/$defs/D{}", i + 1);
        defs.insert(
            format!("D{i}"),
            json!({"type": "object", "properties": {
                "a": {"$ref": next}, "b": {"$ref": next}, "c": {"$ref": next}}}),
        );
    }
    defs.insert("D30".into(), json!({"type": "string"}));
    let s = spec(
        "mcp_lancuch",
        json!({"type": "object", "properties": {"d": {"$ref": "#/$defs/D0"}}, "$defs": defs}),
    );
    let p = tool_parameters(&s);
    assert!(nodes(&p) < 2 * MAX_NODES, "{} węzłów", nodes(&p));
    assert!(jsonschema::validator_for(&p).is_ok());
}

#[test]
fn objects_extended_by_branches_stay_open() {
    let p = tool_parameters(&spec(
        "t",
        json!({"type": "object", "properties": {"a": {"type": "string"}},
               "oneOf": [{"properties": {"b": {"type": "integer"}}, "required": ["b"]},
                         {"required": ["z"]}]}),
    ));
    assert!(
        p.get("additionalProperties").is_none(),
        "gałąź dokłada pole"
    );
    assert!(p["oneOf"][0].get("additionalProperties").is_none());
    assert!(check_arguments(&p, &json!({"a": "x", "b": 1})).is_ok());
    // Alternatywy pod obiektem bez własnych pól są domykane (unie z dyskryminatorem).
    let p = tool_parameters(&spec(
        "t",
        json!({"type": "object", "anyOf": [
            {"properties": {"k": {"const": "a"}, "x": {"type": "string"}}},
            {"properties": {"k": {"const": "b"}, "y": {"type": "integer"}}}]}),
    ));
    assert_eq!(p["anyOf"][0]["additionalProperties"], json!(false));
    assert!(check_arguments(&p, &json!({"k": "a", "y": 1})).is_err());
    // Warunek `if` nie jest domykany; `required` w gałęziach nie otwiera rodzica.
    let p = tool_parameters(&spec(
        "t",
        json!({"type": "object", "properties": {"k": {"type": "string"}, "n": {}},
               "if": {"properties": {"k": {"const": "a"}}}, "then": {"required": ["n"]}}),
    ));
    assert_eq!(p["additionalProperties"], json!(false));
    assert!(p["if"].get("additionalProperties").is_none());
    assert!(check_arguments(&p, &json!({"k": "b"})).is_ok());
    assert!(check_arguments(&p, &json!({"k": "a"})).is_err());
}

#[test]
fn ref_siblings_are_merged() {
    let p = tool_parameters(&spec(
        "t",
        json!({"type": "object",
               "properties": {"o": {"$ref": "#/$defs/O", "description": "Opcje",
                                    "properties": {"extra": {"type": "integer"}},
                                    "required": ["extra"]}},
               "$defs": {"O": {"type": "object", "properties": {"a": {"type": "string"}},
                               "required": ["a"]}}}),
    ));
    let o = &p["properties"]["o"];
    assert_eq!(o["description"], "Opcje");
    assert_eq!(o["required"], json!(["a", "extra"]));
    assert_eq!(o["additionalProperties"], json!(false));
    assert!(check_arguments(&p, &json!({"o": {"a": "x", "extra": 1}})).is_ok());
    assert!(check_arguments(&p, &json!({"o": {"a": "x", "extra": 1, "z": 0}})).is_err());
}

#[test]
fn validation_uses_the_tool_schema_not_the_closed_grammar() {
    // Regresja 0.0.64: domknięcie dla gramatyki nie może odrzucać pól, których narzędzie nie
    // zabrania; narzędzie z `additionalProperties: false` (np. `deny_unknown_fields`) — tak.
    let open = spec(
        "t",
        json!({"type": "object", "properties": {"o": {"$ref": "#/$defs/O"}}, "required": ["o"],
               "$defs": {"O": {"type": "object", "properties": {"a": {"type": "string"}}}}}),
    );
    let extra = json!({"o": {"a": "x", "b": 1}, "c": true});
    assert!(check_arguments(&tool_parameters(&open), &extra).is_err());
    let loose = validation_parameters(&open);
    assert!(loose.get("$defs").is_none(), "odwołania rozwinięte");
    assert!(check_arguments(&loose, &extra).is_ok());
    assert!(check_arguments(&loose, &json!({"o": {"a": 5}})).is_err());
    let strict = spec(
        "s",
        json!({"type": "object", "properties": {"a": {"type": "string"}},
               "additionalProperties": false}),
    );
    let map = validation_of(&[&open, &strict]);
    assert_eq!(map.len(), 2);
    assert!(check_arguments(&map["s"], &json!({"a": "x", "b": 1})).is_err());
}
