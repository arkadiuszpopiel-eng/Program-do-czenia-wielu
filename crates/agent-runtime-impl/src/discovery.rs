//! Katalog jest lokalny; krótkie komendy rozwijają się przed walidacją i detektorem pętli.
use crate::catalog::{CALL, FIND, alias};
use crate::engine::Engine;
use providers_contract::ToolUse;
use serde::Deserialize;
use serde_json::{Value, json};
use tools_common_contract::{ToolErrorKind, ToolOutcome};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallArgs {
    command: String,
    args: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FindArgs {
    query: String,
}

impl Engine {
    pub(crate) fn unknown(&self, name: &str) -> ToolOutcome {
        let hints = self
            .registry
            .find(name, self.cp.verifying)
            .into_iter()
            .take(3)
            .collect::<Vec<_>>();
        let hint = if hints.is_empty() {
            "Brak dopasowania w dostępnych narzędziach.".into()
        } else {
            format!("Najbliższe dostępne: {}.", hints.join(", "))
        };
        ToolOutcome::failed(
            ToolErrorKind::InvalidArgs,
            format!(
                "Nieznane albo niedostępne narzędzie `{name}`. {hint} tools_find(query) pokaże właściwą definicję."
            ),
        )
    }
    pub(crate) fn normalize_calls(&mut self, uses: Vec<ToolUse>) -> Vec<ToolUse> {
        self.rejected_commands.clear();
        uses.into_iter().map(|mut tu| {
            if self.invalid_args.contains_key(&tu.id) { return tu; }
            if tu.name == CALL {
                match serde_json::from_value::<CallArgs>(tu.input.clone()) {
                    Ok(args) if args.args.is_object() && !args.command.trim().is_empty() && args.command.len()<=64 => {
                        let command = args.command.trim().to_lowercase();
                        let name = if self.registry.manifest(&args.command).is_some() { args.command.as_str() } else { alias(&command) };
                        if (name == FIND && !self.registry.names().is_empty()) || self.registry.manifest(name).is_some() {
                            tu.name = name.to_owned();
                            tu.input = args.args;
                        } else {
                            self.rejected_commands.insert(tu.id.clone());
                            self.invalid_args.insert(tu.id.clone(), format!("Niedostępna komenda `{command}`; tools_find(query) wyszukuje wyłącznie dostępne narzędzia."));
                        }
                    }
                    _ => { self.rejected_commands.insert(tu.id.clone()); self.invalid_args.insert(tu.id.clone(), "Podaj command (1–64 znaki) i args jako obiekt zgodny ze schematem narzędzia.".into()); }
                }
            }
            // Pełne wejście (jak lokalne błędy bramki) — liczone przez wykrywanie pętli przed odrzuceniem.
            if let Some(why) = crate::invalid::placeholder(&tu.input).filter(|_| !self.invalid_args.contains_key(&tu.id)) {
                self.rejected_commands.insert(tu.id.clone());
                self.invalid_args.insert(tu.id.clone(), why);
            }
            if let Some(m) = self.registry.manifest(&tu.name) {
                if self.cp.verifying && m.mutating {
                    self.rejected_commands.insert(tu.id.clone());
                    self.invalid_args.insert(tu.id.clone(), "Weryfikacja dopuszcza wyłącznie operacje odczytu.".into());
                } else { self.remember_tools(&[tu.name.clone()]); }
            }
            tu
        }).collect()
    }

    fn remember_tools(&mut self, names: &[String]) {
        for name in names {
            self.cp.discovered_tools.retain(|n| n != name);
            self.cp.discovered_tools.push(name.clone());
        }
        let excess = self.cp.discovered_tools.len().saturating_sub(48);
        self.cp.discovered_tools.drain(..excess);
    }

    pub(crate) fn find_tools(&mut self, input: &Value) -> ToolOutcome {
        let args = match serde_json::from_value::<FindArgs>(input.clone()) {
            Ok(a) if !a.query.trim().is_empty() && a.query.chars().count() <= 512 => a,
            _ => {
                return ToolOutcome::failed(
                    ToolErrorKind::InvalidArgs,
                    "Podaj query: nazwę narzędzia, krótką komendę albo opis (1–512 znaków).",
                );
            }
        };
        let names = self.registry.find(args.query.trim(), self.cp.verifying);
        let items: Vec<_> = names.iter().filter_map(|n| {
            let m = self.registry.manifest(n)?;
            Some(json!({"name":m.name,"title":m.title,"description":m.description.chars().take(180).collect::<String>(),"read_only":!m.mutating}))
        }).collect();
        self.remember_tools(&names);
        let data = json!({"count":items.len(),"tools":items,"schemas_available_next_turn":true});
        let text = if names.is_empty() {
            "Brak dopasowania w dostępnych narzędziach. Spróbuj krótszej nazwy albo kategorii PL/EN.".to_owned()
        } else {
            data.to_string()
        };
        ToolOutcome::ok(text, data)
    }
}
