//! Teksty okna Sędziego Cieni po polsku: rodzaj działania, miejsce (plik, folder, strona,
//! program) zamiast technicznego zapisu zdolności, źródło polecenia, odwracalność, tryby pracy
//! i opis „Co to jest Sędzia Cieni?” pod przyciskiem „?”.

use risk_classifier_contract::{AutonomyLevel, CommandOrigin, Reversibility, RuleId};
use safety_broker_contract::{
    AdminOp, AutonomyTarget, Capability, KernelPolicy, ModelScope, ServiceAction,
};

use crate::card::sanitize;

/// Nazwa okna zgód widoczna dla właściciela (właściciel 31.10: „Sędzia Cieni”).
pub const JUDGE_NAME: &str = "Sędzia Cieni";

/// Treść pod „?”: co robi Sędzia Cieni i do czego się odnosi.
pub const JUDGE_HELP: &str = "Sędzia Cieni to osobny program-strażnik Alfy (Jądro bezpieczeństwa). \
Każde działanie agentki na Twoim komputerze — pliki, polecenia, programy, sieć, ustawienia \
systemu — najpierw trafia do niego. Sędzia ocenia ryzyko według trybu pracy z menu „Tryb pracy” \
w pasku Alfy i albo przepuszcza działanie, albo pyta Cię w tym oknie. Zgodę dajesz tylko tutaj, \
myszą albo klawiszem — agentka ani strona internetowa nie mogą kliknąć za Ciebie. Twarde blokady \
Jądra (np. hasła przeglądarek, wyłączenie dziennika) działają zawsze. Przy „Pełnym dostępie” to \
okno nie pokazuje się wcale, aż obniżysz tryb. „Zawsze w tym zakresie” pozwala na ten sam rodzaj \
działań w tym samym miejscu przez 1 h, 3 h, dobę albo do odwołania — listę zgód i „Odwołaj” \
znajdziesz w Ustawienia → Uprawnienia. Wszystko zapisuje się w dzienniku działań, \
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
        Reversibility::No => "nie da się cofnąć",
    }
}

/// Nazwa trybu pracy taka jak w menu „Tryb pracy” w pasku Alfy.
pub fn mode_name_pl(l: AutonomyLevel) -> &'static str {
    match l {
        AutonomyLevel::L0 => "Plan",
        AutonomyLevel::L1 => "Ręcznie",
        AutonomyLevel::L2 => "Akceptuj edycje",
        AutonomyLevel::L3 => "Automatycznie",
        AutonomyLevel::L4 => "Pełny dostęp",
    }
}

/// Co zmienia tryb — jedno zdanie do okna zgody.
pub fn mode_note_pl(l: AutonomyLevel) -> &'static str {
    match l {
        AutonomyLevel::L0 => "agentki tylko czytają i podpowiadają, nic nie zmieniają",
        AutonomyLevel::L1 => "każda zmiana czeka na Twoje „tak”",
        AutonomyLevel::L2 => {
            "zmieniają pliki w folderze pracy same; pytają o resztę, usuwanie, wysyłkę i instalację"
        }
        AutonomyLevel::L3 => "pracują same; pytają przy nieodwracalnych i po niezaufanej treści",
        AutonomyLevel::L4 => {
            "pracują same i o nic nie pytają — to okno nie pokaże się, aż obniżysz tryb"
        }
    }
}

/// Powód pytania po ludzku (zamiast skrótów klasyfikatora, przegląd właściciela 31.10).
pub fn reason_pl(rule: RuleId) -> &'static str {
    match rule {
        RuleId::KernelBlock => "To obszar chroniony przez Jądro Alfy.",
        RuleId::VoiceDestructive => {
            "Polecenie głosowe zmienia coś bez możliwości cofnięcia — potwierdź kliknięciem."
        }
        RuleId::VoiceLowConfidence => "Rozpoznanie mowy było niepewne.",
        RuleId::VoiceUnverifiedRisky => "Głos nie został rozpoznany jako Twój.",
        RuleId::AdminConsent => "To operacja administratora komputera.",
        RuleId::Trifecta => {
            "Agentka ma dostęp do Twoich danych, czytała niezaufaną treść i chce coś wysłać."
        }
        RuleId::TaintedEgress => {
            "Po kontakcie z niezaufaną treścią (strona, mail, plik) agentka chce wysłać dane na zewnątrz."
        }
        RuleId::MutationNeedsYes => "W tym trybie każda zmiana czeka na Twoje „tak”.",
        RuleId::RiskyAtL2 => "W tym trybie usuwanie, wysyłka i instalacja czekają na Twoją zgodę.",
        RuleId::OutsideWorkdir => {
            "W trybie „Akceptuj edycje” zmiany poza folderem pracy rozmowy czekają na Twoją zgodę."
        }
        RuleId::ProtectedPath => {
            "To ścieżka chroniona (np. .git, .alfa, .ssh albo plik startowy powłoki) — zmiana zawsze czeka na Twoją zgodę."
        }
        RuleId::TaintedHighRisk => {
            "Sesja widziała niezaufaną treść, a to działanie jest ryzykowne."
        }
        RuleId::UntrustedSource => "Polecenie pochodzi z niezaufanej treści, nie od Ciebie.",
        RuleId::IrreversibleOutside => {
            "Tego nie da się cofnąć, a działanie wychodzi poza katalog tej rozmowy."
        }
        RuleId::CriticalRisk => "Ryzyko jest krytyczne.",
        RuleId::EgressNotAllowlisted => "Tej strony nie ma na liście zaufanych adresów.",
        RuleId::GuiOutsideApps => {
            "Tego programu nie ma na liście aplikacji, którymi agentki sterują."
        }
    }
}

/// „Dlaczego pytam”: powody z reguł (bez powtórzeń; bez reguł — opis od klasyfikatora) i tryb,
/// przy którym padło pytanie.
pub fn why_pl(rules: &[RuleId], explanation: &str, level: Option<AutonomyLevel>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for r in rules {
        let text = reason_pl(*r).to_owned();
        if !parts.contains(&text) {
            parts.push(text);
        }
    }
    if parts.is_empty() && !explanation.is_empty() {
        parts.push(sanitize(explanation, 400));
    }
    if let Some(l) = level {
        parts.push(format!("Tryb pracy: „{}”.", mode_name_pl(l)));
    }
    parts.join(" ")
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
        AutonomyTarget::Models { scope } => scope_pl(*scope).into(),
    }
}

/// Zasięg „Max dostęp” po polsku (okno Sędziego, plan 50 §8.1).
pub fn scope_pl(scope: ModelScope) -> &'static str {
    match scope {
        ModelScope::Local => "Max dostęp: agentki na modelach lokalnych",
        ModelScope::Cloud => "Max dostęp: agentki na modelach w chmurze",
        ModelScope::All => "Max dostęp: wszystkie agentki, każdy model",
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
