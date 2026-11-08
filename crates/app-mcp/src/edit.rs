//! Dodawanie, zmiana i usuwanie serwerów (formularz w Ustawieniach). Wartości tajne trafiają do
//! Menedżera poświadczeń; zmiana polecenia, argumentów albo katalogu unieważnia zgody na narzędzia
//! (pod tą samą nazwą może działać już inny program).

use std::collections::BTreeMap;

use accounts_hub_contract::SecretString;
use app_api::AppError;
use app_api::dto::{McpEnvVar, McpServerInput, McpServerView};

use crate::hub::{McpHub, secret_name};
use crate::store::{MAX_SERVERS, ServerEntry, looks_secret, save_servers, slug, valid_env_name};
use crate::views::to_trust;

/// Najdłuższy dopuszczalny limit czasu żądania (10 min).
const MAX_TIMEOUT_MS: u64 = 10 * 60 * 1000;
/// Najkrótszy sensowny limit czasu żądania.
const MIN_TIMEOUT_MS: u64 = 1_000;

fn invalid(field: &str, message: impl Into<String>) -> AppError {
    AppError::invalid_field(field, message)
}

/// Sprawdzona zmienna: nazwa, wartość jawna, wartość tajna (nowa) albo zachowanie starej.
struct EnvPlan {
    plain: BTreeMap<String, String>,
    secret: Vec<(String, Option<String>)>,
}

fn plan_env(vars: &[McpEnvVar], old: Option<&ServerEntry>) -> Result<EnvPlan, AppError> {
    let mut plan = EnvPlan {
        plain: BTreeMap::new(),
        secret: Vec::new(),
    };
    let mut seen = std::collections::BTreeSet::new();
    for v in vars {
        let name = v.name.trim().to_owned();
        if !valid_env_name(&name) {
            return Err(invalid(
                "env",
                format!("Nazwa zmiennej `{name}` jest niepoprawna (litery, cyfry, `_`)."),
            ));
        }
        if !seen.insert(name.to_ascii_uppercase()) {
            return Err(invalid(
                "env",
                format!("Zmienna {name} występuje dwa razy."),
            ));
        }
        let value = v.value.clone().filter(|s| !s.is_empty());
        if v.secret || looks_secret(&name) {
            let kept = old.is_some_and(|o| o.secret_env.contains(&name));
            if value.is_none() && !kept {
                return Err(invalid(
                    "env",
                    format!("Podaj wartość tajnej zmiennej {name}."),
                ));
            }
            plan.secret.push((name, value));
        } else {
            plan.plain.insert(name, value.unwrap_or_default());
        }
    }
    Ok(plan)
}

impl McpHub {
    /// Zapisuje serwer (nowy, gdy `id` puste). Połączenie jest zamykane — nowe ustawienia działają
    /// od następnego użycia.
    pub async fn save(&self, input: McpServerInput) -> Result<McpServerView, AppError> {
        let name = input.name.trim().to_owned();
        let command = input.command.trim().to_owned();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(invalid("name", "Podaj nazwę serwera (do 80 znaków)."));
        }
        if command.is_empty() {
            return Err(invalid("command", "Podaj polecenie uruchamiające serwer."));
        }
        if input.args.len() > 64 || input.args.iter().any(|a| a.len() > 4096) {
            return Err(invalid("args", "Za dużo albo za długie argumenty."));
        }
        let timeout_ms = input
            .timeout_ms
            .unwrap_or(mcp_contract::client::DEFAULT_REQUEST_TIMEOUT_MS)
            .clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS);
        let cwd = input
            .cwd
            .map(|c| c.trim().to_owned())
            .filter(|c| !c.is_empty());
        if let Some(dir) = &cwd
            && !std::path::Path::new(dir).is_dir()
        {
            return Err(invalid("cwd", format!("Katalog `{dir}` nie istnieje.")));
        }
        let old = input.id.as_deref().and_then(|id| self.entry(id));
        if input.id.is_some() && old.is_none() {
            return Err(AppError::not_found("serwer MCP"));
        }
        let env = plan_env(&input.env, old.as_ref())?;
        if env.secret.iter().any(|(_, v)| v.is_some()) && self.secrets.is_none() {
            return Err(AppError::unavailable("zmienne tajne", "accounts-hub"));
        }
        let id = match &old {
            Some(o) => o.id.clone(),
            None => {
                let entries = self.lock_entries();
                if entries.len() >= MAX_SERVERS {
                    return Err(invalid(
                        "name",
                        format!("Najwyżej {MAX_SERVERS} serwerów MCP."),
                    ));
                }
                let base = slug(&name);
                let mut id = base.clone();
                let mut n = 2;
                while entries.iter().any(|e| e.id == id) {
                    id = format!("{base}-{n}");
                    n += 1;
                }
                id
            }
        };
        for (var, value) in &env.secret {
            if let (Some(value), Some(store), Some(sname)) =
                (value, &self.secrets, secret_name(&id, var))
            {
                store
                    .put(&sname, &SecretString::from_input(value))
                    .map_err(|e| AppError::new(app_api::ErrorCode::Secrets, e.to_string()))?;
            }
        }
        let secret_env: Vec<String> = env.secret.iter().map(|(n, _)| n.clone()).collect();
        if let Some(o) = &old {
            self.drop_secrets(
                &o.id,
                o.secret_env.iter().filter(|v| !secret_env.contains(v)),
            );
        }
        let entry = ServerEntry {
            id: id.clone(),
            name,
            command,
            args: input.args,
            cwd,
            env: env.plain,
            secret_env,
            trust: to_trust(input.trust),
            enabled: input.enabled,
            timeout_ms,
        };
        let program_changed = old.as_ref().is_some_and(|o| {
            o.command != entry.command || o.args != entry.args || o.cwd != entry.cwd
        });
        {
            let mut entries = self.lock_entries();
            let mut next = entries.clone();
            match next.iter_mut().find(|e| e.id == id) {
                Some(slot) => *slot = entry.clone(),
                None => next.push(entry.clone()),
            }
            save_servers(&self.servers_path, &next).map_err(AppError::storage)?;
            *entries = next;
        }
        if program_changed {
            self.pins.forget_server(&id);
        }
        self.disconnect(&id).await;
        self.lock_status().remove(&id);
        Ok(self.view(&entry, false))
    }

    fn drop_secrets<'a>(&self, id: &str, vars: impl Iterator<Item = &'a String>) {
        let Some(store) = &self.secrets else {
            return;
        };
        for var in vars {
            if let Some(name) = secret_name(id, var) {
                let _ = store.delete(&name);
            }
        }
    }

    /// Usuwa serwer, jego zgody i wartości tajne.
    pub async fn remove(&self, id: &str) -> Result<(), AppError> {
        let removed = {
            let mut entries = self.lock_entries();
            let Some(pos) = entries.iter().position(|e| e.id == id) else {
                return Err(AppError::not_found("serwer MCP"));
            };
            let mut next = entries.clone();
            let removed = next.remove(pos);
            save_servers(&self.servers_path, &next).map_err(AppError::storage)?;
            *entries = next;
            removed
        };
        self.disconnect(id).await;
        self.pins.forget_server(id);
        self.drop_secrets(id, removed.secret_env.iter());
        self.lock_status().remove(id);
        Ok(())
    }
}
