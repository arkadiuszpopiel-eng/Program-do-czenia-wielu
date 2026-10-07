//! JSON-RPC 2.0 — wspólny dla MCP (stdio: jedna wiadomość JSON na linię) i dla trybu
//! `codex app-server` (ten sam format, ale bez pola `"jsonrpc"` — tryb tolerancyjny).
//!
//! Paczki (tablice) są odrzucane: MCP 2025-06-18 usunęło batching.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Wersja protokołu w polu `"jsonrpc"`.
pub const JSONRPC_VERSION: &str = "2.0";
/// Niepoprawny JSON.
pub const PARSE_ERROR: i64 = -32700;
/// Niepoprawne żądanie (np. brak `method`, zły typ `id`, paczka).
pub const INVALID_REQUEST: i64 = -32600;
/// Nieznana metoda.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// Niepoprawne parametry (także nieznane narzędzie — wg MCP).
pub const INVALID_PARAMS: i64 = -32602;
/// Błąd wewnętrzny.
pub const INTERNAL_ERROR: i64 = -32603;
/// Żądanie przed `initialize` (zakres błędów serwera -32000..-32099).
pub const NOT_INITIALIZED: i64 = -32002;
/// Brak uprawnień (narzędzie spoza zakresu rejestracji, odmowa polityki).
pub const UNAUTHORIZED: i64 = -32001;

/// Identyfikator żądania (liczba albo tekst; `null` nie jest dozwolony w żądaniach MCP).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RequestId {
    /// Liczbowy.
    Number(i64),
    /// Tekstowy.
    Text(String),
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::Number(n) => write!(f, "{n}"),
            RequestId::Text(s) => f.write_str(s),
        }
    }
}

/// Obiekt błędu JSON-RPC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RpcError {
    /// Kod.
    pub code: i64,
    /// Komunikat (bez sekretów).
    pub message: String,
    /// Dane dodatkowe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    /// Błąd z kodem i komunikatem.
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Nieznana metoda.
    pub fn method_not_found(method: &str) -> Self {
        Self::new(METHOD_NOT_FOUND, format!("nieznana metoda `{method}`"))
    }

    /// Niepoprawne parametry.
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, message)
    }
}

/// Wiadomość JSON-RPC po klasyfikacji.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// Żądanie (oczekuje odpowiedzi).
    Request {
        /// Identyfikator.
        id: RequestId,
        /// Metoda.
        method: String,
        /// Parametry.
        params: Option<Value>,
    },
    /// Powiadomienie (bez odpowiedzi).
    Notification {
        /// Metoda.
        method: String,
        /// Parametry.
        params: Option<Value>,
    },
    /// Odpowiedź na żądanie.
    Response {
        /// Identyfikator żądania.
        id: RequestId,
        /// Wynik albo błąd.
        outcome: Result<Value, RpcError>,
    },
}

/// Błąd parsowania wiadomości: identyfikator (jeśli dało się go odczytać) i obiekt błędu do odesłania.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseFailure {
    /// Identyfikator żądania, jeśli był czytelny.
    pub id: Option<RequestId>,
    /// Błąd do odesłania.
    pub error: RpcError,
}

/// Czy wymagać pola `"jsonrpc": "2.0"` (MCP — tak; `codex app-server` — nie).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// Pełny JSON-RPC 2.0 (MCP).
    Strict,
    /// Bez nagłówka `"jsonrpc"` (codex app-server); nagłówek, jeśli jest, musi mieć wartość `2.0`.
    Lenient,
}

fn failure(id: Option<RequestId>, code: i64, message: &str) -> ParseFailure {
    ParseFailure {
        id,
        error: RpcError::new(code, message),
    }
}

fn parse_id(value: &Value) -> Option<RequestId> {
    match value {
        Value::Number(n) => n.as_i64().map(RequestId::Number),
        Value::String(s) => Some(RequestId::Text(s.clone())),
        _ => None,
    }
}

/// Parsuje jedną linię transportu.
pub fn parse_line(line: &str, dialect: Dialect) -> Result<Message, ParseFailure> {
    let value: Value = serde_json::from_str(line.trim())
        .map_err(|e| failure(None, PARSE_ERROR, &format!("niepoprawny JSON: {e}")))?;
    parse_value(value, dialect)
}

/// Klasyfikuje wartość JSON jako wiadomość JSON-RPC.
pub fn parse_value(value: Value, dialect: Dialect) -> Result<Message, ParseFailure> {
    let Value::Object(mut obj) = value else {
        return Err(failure(
            None,
            INVALID_REQUEST,
            "wiadomość nie jest obiektem (paczki nie są obsługiwane)",
        ));
    };
    let raw_id = obj.remove("id");
    let id = match &raw_id {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            parse_id(v).ok_or_else(|| failure(None, INVALID_REQUEST, "niepoprawny typ `id`"))?,
        ),
    };
    match obj.get("jsonrpc") {
        Some(Value::String(v)) if v == JSONRPC_VERSION => {}
        None if dialect == Dialect::Lenient => {}
        _ => {
            return Err(failure(
                id,
                INVALID_REQUEST,
                "brak lub zła wersja `jsonrpc`",
            ));
        }
    }
    if let Some(method) = obj.remove("method") {
        let Value::String(method) = method else {
            return Err(failure(id, INVALID_REQUEST, "`method` nie jest tekstem"));
        };
        let params = obj.remove("params");
        if matches!(params, Some(ref p) if !p.is_object() && !p.is_array()) {
            return Err(failure(id, INVALID_REQUEST, "`params` musi być obiektem"));
        }
        return Ok(match id {
            Some(id) => Message::Request { id, method, params },
            None if raw_id.is_some() => {
                return Err(failure(None, INVALID_REQUEST, "`id` nie może być null"));
            }
            None => Message::Notification { method, params },
        });
    }
    let Some(id) = id else {
        return Err(failure(None, INVALID_REQUEST, "odpowiedź bez `id`"));
    };
    let outcome = match (obj.remove("result"), obj.remove("error")) {
        (Some(result), None) => Ok(result),
        (None, Some(error)) => Err(serde_json::from_value::<RpcError>(error)
            .map_err(|e| failure(Some(id.clone()), INVALID_REQUEST, &e.to_string()))?),
        _ => {
            return Err(failure(
                Some(id),
                INVALID_REQUEST,
                "odpowiedź musi mieć dokładnie jedno z pól `result`/`error`",
            ));
        }
    };
    Ok(Message::Response { id, outcome })
}

impl Message {
    /// Żądanie.
    pub fn request(id: RequestId, method: impl Into<String>, params: Option<Value>) -> Self {
        Self::Request {
            id,
            method: method.into(),
            params,
        }
    }

    /// Powiadomienie.
    pub fn notification(method: impl Into<String>, params: Option<Value>) -> Self {
        Self::Notification {
            method: method.into(),
            params,
        }
    }

    /// Odpowiedź z wynikiem.
    pub fn result(id: RequestId, result: Value) -> Self {
        Self::Response {
            id,
            outcome: Ok(result),
        }
    }

    /// Odpowiedź z błędem.
    pub fn error(id: RequestId, error: RpcError) -> Self {
        Self::Response {
            id,
            outcome: Err(error),
        }
    }

    /// Wartość JSON; `dialect == Strict` dodaje `"jsonrpc": "2.0"`.
    pub fn to_value(&self, dialect: Dialect) -> Value {
        let mut obj = Map::new();
        if dialect == Dialect::Strict {
            obj.insert("jsonrpc".into(), Value::String(JSONRPC_VERSION.into()));
        }
        match self {
            Message::Request { id, method, params } => {
                obj.insert("id".into(), id_value(id));
                obj.insert("method".into(), Value::String(method.clone()));
                if let Some(p) = params {
                    obj.insert("params".into(), p.clone());
                }
            }
            Message::Notification { method, params } => {
                obj.insert("method".into(), Value::String(method.clone()));
                if let Some(p) = params {
                    obj.insert("params".into(), p.clone());
                }
            }
            Message::Response { id, outcome } => {
                obj.insert("id".into(), id_value(id));
                match outcome {
                    Ok(v) => obj.insert("result".into(), v.clone()),
                    Err(e) => obj.insert("error".into(), error_value(e)),
                };
            }
        }
        Value::Object(obj)
    }

    /// Jedna linia transportu (bez końcowego `\n`; serde_json nie wstawia znaków nowej linii).
    pub fn to_line(&self, dialect: Dialect) -> String {
        self.to_value(dialect).to_string()
    }
}

/// Odpowiedź z błędem dla żądania, którego `id` nie dało się odczytać (`"id": null`).
pub fn error_line_without_id(error: &RpcError, dialect: Dialect) -> String {
    let mut obj = Map::new();
    if dialect == Dialect::Strict {
        obj.insert("jsonrpc".into(), Value::String(JSONRPC_VERSION.into()));
    }
    obj.insert("id".into(), Value::Null);
    obj.insert("error".into(), error_value(error));
    Value::Object(obj).to_string()
}

fn id_value(id: &RequestId) -> Value {
    match id {
        RequestId::Number(n) => Value::from(*n),
        RequestId::Text(s) => Value::String(s.clone()),
    }
}

fn error_value(error: &RpcError) -> Value {
    serde_json::to_value(error).unwrap_or_else(
        |_| serde_json::json!({"code": INTERNAL_ERROR, "message": "błąd serializacji"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classifies_messages() {
        let req = parse_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
            Dialect::Strict,
        )
        .unwrap();
        assert!(matches!(req, Message::Request { ref method, .. } if method == "tools/list"));
        let note = parse_line(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            Dialect::Strict,
        )
        .unwrap();
        assert!(matches!(note, Message::Notification { .. }));
        let resp =
            parse_line(r#"{"jsonrpc":"2.0","id":"a","result":{}}"#, Dialect::Strict).unwrap();
        assert_eq!(
            resp,
            Message::result(RequestId::Text("a".into()), json!({}))
        );
        let err = parse_line(
            r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"x"}}"#,
            Dialect::Strict,
        )
        .unwrap();
        assert!(
            matches!(err, Message::Response { outcome: Err(ref e), .. } if e.code == METHOD_NOT_FOUND)
        );
    }

    #[test]
    fn rejects_malformed() {
        assert_eq!(
            parse_line("{", Dialect::Strict).unwrap_err().error.code,
            PARSE_ERROR
        );
        assert_eq!(
            parse_line("[]", Dialect::Strict).unwrap_err().error.code,
            INVALID_REQUEST
        );
        let no_header = r#"{"id":1,"method":"x"}"#;
        assert_eq!(
            parse_line(no_header, Dialect::Strict).unwrap_err().id,
            Some(RequestId::Number(1))
        );
        assert!(parse_line(no_header, Dialect::Lenient).is_ok());
        let bad_id = r#"{"jsonrpc":"2.0","id":{},"method":"x"}"#;
        assert_eq!(
            parse_line(bad_id, Dialect::Strict).unwrap_err().error.code,
            INVALID_REQUEST
        );
        let null_id = r#"{"jsonrpc":"2.0","id":null,"method":"x"}"#;
        assert!(parse_line(null_id, Dialect::Strict).is_err());
        let both = r#"{"jsonrpc":"2.0","id":1,"result":1,"error":{"code":1,"message":"m"}}"#;
        assert!(parse_line(both, Dialect::Strict).is_err());
        let bad_params = r#"{"jsonrpc":"2.0","id":1,"method":"x","params":3}"#;
        assert!(parse_line(bad_params, Dialect::Strict).is_err());
    }

    #[test]
    fn round_trips_lines() {
        let msg = Message::request(RequestId::Number(7), "ping", Some(json!({})));
        let line = msg.to_line(Dialect::Strict);
        assert!(!line.contains('\n'));
        assert_eq!(parse_line(&line, Dialect::Strict).unwrap(), msg);
        let lenient = msg.to_line(Dialect::Lenient);
        assert!(!lenient.contains("jsonrpc"));
        assert_eq!(parse_line(&lenient, Dialect::Lenient).unwrap(), msg);
        let e = Message::error(RequestId::Number(1), RpcError::method_not_found("x"));
        assert_eq!(
            parse_line(&e.to_line(Dialect::Strict), Dialect::Strict).unwrap(),
            e
        );
        let null = error_line_without_id(&RpcError::new(PARSE_ERROR, "p"), Dialect::Strict);
        assert!(null.contains("\"id\":null"));
        assert_eq!(RequestId::Text("x".into()).to_string(), "x");
    }
}
