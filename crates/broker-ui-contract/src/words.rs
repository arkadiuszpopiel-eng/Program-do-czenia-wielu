//! Teksty okna Sędziego Cieni po polsku: rodzaj działania, miejsce (plik, folder, strona,
//! program) zamiast technicznego zapisu zdolności, źródło polecenia, odwracalność, tryby pracy
//! i opis „Co to jest Sędzia Cieni?” pod przyciskiem „?”.

use risk_classifier_contract::{AutonomyLevel, CommandOrigin, Reversibility};
use safety_broker_contract::{AdminOp, AutonomyTarget, Capability, KernelPolicy, ServiceAction};

use crate::card::sanitize;

/// Nazwa okna zgód widoczna dla właściciela (właściciel 31.10: „Sędzia Cieni”).
pub const JUDGE_NAME: &str = "Sędzia Cieni";

/// Treść pod „?”: co robi Sędzia Cieni i do czego się odnosi.
pub const JUDGE_HELP: &str = "Sędzia Cieni to osobny program-strażnik Alfy (Jądro bezpieczeństwa). \
Każde działanie agentki na Twoim komputerze — pliki, polecenia, programy, sieć, ustawienia \
systemu — najpierw trafia do niego. Sędzia ocenia ryzyko według trybu pracy z menu „Tryb pracy” \
w pasku Alfy i albo przepuszcza działanie, albo pyta Cię w tym oknie. Zgodę dajesz tylko tutaj, \
myszą albo klawiszem — agentka ani strona internetowa nie mogą kliknąć za Ciebie. Twarde blokady \
Jądra (np. hasła przeglądarek, wyłączenie dziennika) działają zawsze. Na „Pełnej automatyce” to \
okno nie pokazuje się wcale, aż obniżysz tryb. Wszystko zapisuje się w dzienniku działań, \
a Ctrl+Shift+F12 zatrzymuje agentki natychmiast.";

/// Rodzaj działania (rodzina zdolności).
pub fn verb(cap: &Capability) -> &'static str {
    match cap {
        Capability::FsRead(_) => "odczyt plików",
        Capability::FsWrite(_) => "zmiana lub usunięcie plików",
        Capability::ShellExec(_) => "polecenie powłoki",
        Capability::GuiControl(_) => "sterowanie aplikacją",
        Capability::NetEgress(_) => "połączenie z internetem",
        Capability::SecretsRead(_) => "odczyt sekretu",
        Capability::SystemAdmin(_) => "operacja administratora",
    }
}

fn service_pl(a: &ServiceAction) -> &'static str {
    match a {
        ServiceAction::Start => "uruchomienie",
        ServiceAction::Stop => "zatrzymanie",
        ServiceAction::Restart => "ponowne uruchomienie",
        ServiceAction::Disable => "wyłączenie autostartu",
        ServiceAction::Delete => "usunięcie",
        ServiceAction::Configure => "zmiana konfiguracji",
    }
}

fn admin_pl(op: &AdminOp) -> String {
    let v = |s: &str| sanitize(s, 200);
    match op {
        AdminOp::Install { package } => format!("instalacja programu „{}”", v(package)),
        AdminOp::ServiceControl { service, action } => {
            format!("{} usługi Windows „{}”", service_pl(action), v(service))
        }
        AdminOp::Firewall { rule } => format!("reguła zapory „{}”", v(rule)),
        AdminOp::RegistryMachine { key } => format!("rejestr komputera: {}", v(key)),
        AdminOp::DisableAudit => "wyłączenie dziennika zdarzeń".into(),
        AdminOp::FormatDisk { drive } => format!("formatowanie dysku {drive}:"),
        AdminOp::Bootloader => "zmiana programu rozruchowego".into(),
        AdminOp::ChangeKernelPolicy => "zmiana zasad Jądra".into(),
        AdminOp::Other { command } => format!("polecenie administratora: {}", v(command)),
    }
}

/// Miejsce działania po ludzku: plik/folder, strona, program, sekret, operacja.
pub fn place_pl(cap: &Capability) -> String {
    let v = |s: &str| sanitize(s, 260);
    match cap {
        Capability::FsRead(s) | Capability::FsWrite(s) | Capability::ShellExec(s) => {
            if s.subtree() {
                format!("folder {} (z podfolderami)", v(s.canonical()))
            } else {
                v(s.canonical())
            }
        }
        Capability::GuiControl(app) => format!("program {}", v(&app.to_string())),
        Capability::NetEgress(h) if h.subdomains() => {
            format!("strona {} (z subdomenami)", v(h.host()))
        }
        Capability::NetEgress(h) => format!("strona {}", v(h.host())),
        Capability::SecretsRead(id) => format!("sekret „{}”", v(id.as_str())),
        Capability::SystemAdmin(op) => admin_pl(op),
    }
}

/// Źródło polecenia.
pub fn origin_pl(o: &CommandOrigin) -> &'static str {
    match o {
        CommandOrigin::UserText => "Ty (tekst)",
        CommandOrigin::UserVoice { .. } => "Ty (głos) — potwierdź kliknięciem lub klawiszem",
        CommandOrigin::Agent => "agentka (z własnej inicjatywy)",
        CommandOrigin::UntrustedContent => "NIEZAUFANA TREŚĆ (strona, mail, plik, dźwięk)",
    }
}

/// Odwracalność.
pub fn reversible_pl(r: Reversibility) -> &'static str {
    match r {
        Reversibility::Yes => "da się cofnąć (dziennik cofania)",
        Reversibility::Scoped => "da się cofnąć w zakresie (migawka przed wykonaniem)",
        Reversibility::No => "NIE da się cofnąć",
    }
}

/// Nazwa trybu pracy taka jak w menu „Tryb pracy” w pasku Alfy.
pub fn mode_name_pl(l: AutonomyLevel) -> &'static str {
    match l {
        AutonomyLevel::L0 => "Tylko podgląd",
        AutonomyLevel::L1 => "Pytaj o wszystko",
        AutonomyLevel::L2 => "Pytaj o ryzykowne",
        AutonomyLevel::L3 => "Automatycznie",
        AutonomyLevel::L4 => "Pełna automatyka",
    }
}

/// Co zmienia tryb — jedno zdanie do okna zgody.
pub fn mode_note_pl(l: AutonomyLevel) -> &'static str {
    match l {
        AutonomyLevel::L0 => "agentki tylko czytają i podpowiadają, nic nie zmieniają",
        AutonomyLevel::L1 => "każda zmiana czeka na Twoje „tak”",
        AutonomyLevel::L2 => "drobne rzeczy same; pytają przy usuwaniu, wysyłce i instalacji",
        AutonomyLevel::L3 => "pracują same; pytają przy nieodwracalnych i po niezaufanej treści",
        AutonomyLevel::L4 => {
            "pracują same i o nic nie pytają — to okno nie pokaże się, aż obniżysz tryb"
        }
    }
}

/// Dopisek do „Dlaczego pytam”: tryb, przy którym padło pytanie.
pub fn asked_at_pl(l: AutonomyLevel) -> String {
    format!(
        "Tryb pracy: „{}”. Na „Pełnej automatyce” to okno się nie pokazuje.",
        mode_name_pl(l)
    )
}

/// Cel zmiany trybu.
pub fn target_pl(t: &AutonomyTarget) -> String {
    // Imię agentki z wielkiej litery, jak w nagłówku karty („agentka Delta”).
    let raw = |a: &str| sanitize(a, 40);
    let name = |a: &str| {
        let a = sanitize(a, 40);
        let mut c = a.chars();
        c.next()
            .map(|f| f.to_uppercase().chain(c).collect::<String>())
            .unwrap_or_default()
    };
    match t {
        AutonomyTarget::Global => "cała aplikacja".into(),
        AutonomyTarget::Session { session } => format!("sesja {}", raw(session.as_str())),
        AutonomyTarget::Agent { agent } => format!("agentka {}", name(agent.as_str())),
        AutonomyTarget::SessionAgent { session, agent } => format!(
            "agentka {} w sesji {}",
            name(agent.as_str()),
            raw(session.as_str())
        ),
    }
}

/// Polityka Jądra w jednym wierszu.
pub fn policy_pl(p: &KernelPolicy) -> String {
    format!(
        "TTL tokenu {} min (maks. {} min), egress: {} hostów, aplikacje: {}, Hello dla: {:?}",
        p.token_ttl_default_ms / 60_000,
        p.token_ttl_max_ms / 60_000,
        p.egress_allowlist.len(),
        p.allowed_apps.len(),
        p.hello_required_for
    )
}
