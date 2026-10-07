//! Wiązanie ról IPC z tożsamością procesu po drugiej stronie potoku (PLAN §8.1, §8.4):
//! rola deklarowana w powitaniu musi zgadzać się z kontem, poziomem integralności i obrazem
//! procesu ustalonymi przez system. Broker-UI: wyłącznie wysoki poziom integralności, konkretny
//! obraz i poświadczenie z biletu startowego (nigdy „zapis” bez poświadczenia).

use platform_contract::{IntegrityLevel, PeerIdentity, PeerRequirement};
use safety_broker_contract::ipc::{ClientCredential, ClientRole, Hello};
use serde::{Deserialize, Serialize};

/// Wymagania dla jednej roli.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleBinding {
    /// Tożsamość procesu.
    pub requirement: PeerRequirement,
    /// Czy proces spełniający wymagania może się przedstawić bez poświadczenia z MAC (jądro
    /// i watchdog startują z launchera, nie od Brokera). Nigdy dla Broker-UI.
    #[serde(default)]
    pub enroll: bool,
}

/// Wiązania ról; `None` = rola wyłączona na potoku.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleBindings {
    /// Jądro Alfy.
    pub core: Option<RoleBinding>,
    /// Procesy agentek.
    pub agent: Option<RoleBinding>,
    /// Okno zatwierdzeń.
    pub broker_ui: Option<RoleBinding>,
    /// Watchdog.
    pub watchdog: Option<RoleBinding>,
}

impl RoleBindings {
    /// Wiązanie roli.
    pub fn get(&self, role: ClientRole) -> Option<&RoleBinding> {
        match role {
            ClientRole::Core => self.core.as_ref(),
            ClientRole::Agent => self.agent.as_ref(),
            ClientRole::BrokerUi => self.broker_ui.as_ref(),
            ClientRole::Watchdog => self.watchdog.as_ref(),
        }
    }

    /// Walidacja: konta niepuste; zapis bez poświadczenia tylko dla wskazanych obrazów;
    /// Broker-UI bez zapisu, z obrazem i — poza trybem deweloperskim — z wysoką integralnością.
    pub fn validate(&self, dev_mode: bool) -> Result<(), String> {
        let all = [
            ("core", &self.core),
            ("agent", &self.agent),
            ("broker_ui", &self.broker_ui),
            ("watchdog", &self.watchdog),
        ];
        for (name, b) in all.iter().filter_map(|(n, b)| b.as_ref().map(|b| (*n, b))) {
            if b.requirement.users.is_empty() {
                return Err(format!("{name}: brak dozwolonych kont"));
            }
            if b.enroll && b.requirement.images.is_empty() {
                return Err(format!(
                    "{name}: zapis bez poświadczenia wymaga listy obrazów"
                ));
            }
        }
        if let Some(ui) = &self.broker_ui {
            if ui.enroll {
                return Err("broker_ui: zapis bez poświadczenia jest zabroniony".into());
            }
            if ui.requirement.images.is_empty() {
                return Err("broker_ui: wymagany obraz alfa-broker-ui.exe".into());
            }
            if !dev_mode && ui.requirement.min_integrity < IntegrityLevel::High {
                return Err("broker_ui: wymagany wysoki poziom integralności (UIPI)".into());
            }
        }
        Ok(())
    }

    /// Decyzja o powitaniu: tożsamość procesu musi spełniać wiązanie zadeklarowanej roli, a
    /// poświadczenie (MAC) musi być poprawne — chyba że rola dopuszcza zapis po tożsamości i MAC
    /// jest pusty.
    pub fn authorize(
        &self,
        hello: &Hello,
        peer: &PeerIdentity,
        verify_mac: impl FnOnce(&ClientCredential) -> Result<(), String>,
    ) -> Result<(), String> {
        let role = hello.credential.role;
        let binding = self
            .get(role)
            .ok_or_else(|| format!("rola {role:?} jest wyłączona na tym kanale"))?;
        binding
            .requirement
            .check(peer)
            .map_err(|e| format!("tożsamość procesu {} (rola {role:?}): {e}", peer.pid))?;
        if hello.credential.mac.is_empty() {
            return if binding.enroll {
                Ok(())
            } else {
                Err(format!("rola {role:?} wymaga poświadczenia"))
            };
        }
        verify_mac(&hello.credential)
    }
}
