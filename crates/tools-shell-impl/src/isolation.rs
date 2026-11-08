//! Izolacja poleceń powłoki integralnością Low (dokument 25 §1): które polecenia jej nie dostają
//! i jak wynik to opisuje.

/// Narzędzia, które piszą w profilu użytkownika (pamięć podręczna pakietów, rejestry) — pod
/// izolacją Low dostałyby odmowę, więc ich polecenia idą jak dotąd (Medium, za zgodami).
const PROFILE_TOOLCHAINS: [&str; 22] = [
    "cargo", "rustup", "npm", "npx", "pnpm", "yarn", "bun", "deno", "pip", "pip3", "uv", "poetry",
    "conda", "go", "dotnet", "nuget", "gradle", "gradlew", "mvn", "choco", "winget", "scoop",
];

pub(crate) fn uses_profile_toolchain(command: &str) -> bool {
    command
        .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == '.'))
        .map(|w| {
            w.trim_end_matches(".exe")
                .trim_end_matches(".cmd")
                .to_lowercase()
        })
        .any(|w| PROFILE_TOOLCHAINS.contains(&w.as_str()))
}

/// Jak polecenie zostało odizolowane (dopisek w wyniku dla modelu).
pub(crate) enum Isolation {
    Off,
    Low,
    Unavailable(String),
}
