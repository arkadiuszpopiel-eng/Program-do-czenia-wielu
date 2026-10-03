//! Procesy powiązane z oknem-celem (przegląd bezpieczeństwa #2, P2-01). Okno najwyższego poziomu
//! nie zawsze należy do procesu, który je „naprawdę” pokazuje albo kontroluje:
//!
//! - wyskakujące okna WebView2 Alfy (lista `<select>`, menu kontekstowe, okno przeglądarki) należą
//!   do `msedgewebview2.exe` — procesu potomnego Alfy, którego obraz leży poza katalogami Alfy;
//! - okna-własności (dialog, menu) mają właściciela w innym oknie (łańcuch `GA_ROOTOWNER`);
//! - okna UWP to ramka `ApplicationFrameHost.exe`, a treść pokazuje proces aplikacji z okna
//!   potomnego `Windows.UI.Core.CoreWindow` (ramka bez rozpoznanej treści = proces nieznany).
//!
//! Każdy taki proces jest opisany [`ProcessLink`] z łańcuchem przodków **odczytanym w chwili
//! sprawdzenia** (drzewo procesów Alfy liczone przy każdej akcji, nie tylko przy starcie —
//! proces WebView2 odtworzony po awarii renderera też jest chroniony). Okno jest chronione, gdy
//! chroniony jest którykolwiek powiązany proces (fail-closed).
//!
//! Testy: `platform-fake/tests/review_contract.rs` (limit rozmiaru crate'a kontraktu).

use serde::{Deserialize, Serialize};

use crate::gui::{GuiError, TargetGuard, image_file_name};

/// Klasa ramki okien UWP (`ApplicationFrameHost.exe`).
pub const UWP_FRAME_CLASS: &str = "ApplicationFrameWindow";
/// Klasa okna treści aplikacji UWP (dziecko ramki, proces aplikacji).
pub const UWP_CORE_CLASS: &str = "Windows.UI.Core.CoreWindow";
/// Obraz hosta ramek UWP.
pub const UWP_FRAME_HOST: &str = "applicationframehost.exe";
/// Najdłuższy uwzględniany łańcuch przodków (ochrona przed cyklem po ponownym użyciu PID).
pub const MAX_ANCESTORS: usize = 64;

/// Rola procesu wobec okna-celu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkRole {
    /// Proces okna najwyższego poziomu (`GA_ROOT`).
    #[default]
    Root,
    /// Proces samego okna (np. okno potomne innego procesu).
    Window,
    /// Proces właściciela (`GA_ROOTOWNER`, łańcuch okien-własności).
    Owner,
    /// Proces aplikacji UWP (okno `CoreWindow` w ramce).
    UwpContent,
    /// Ramka UWP bez rozpoznanej treści (aplikacja zawieszona/zminimalizowana) — nieznany.
    UwpUnresolved,
}

/// Proces powiązany z oknem-celem.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessLink {
    /// Rola.
    pub role: LinkRole,
    /// PID (0 = nieznany).
    pub pid: u32,
    /// Obraz procesu (pełna ścieżka; pusty = nieznany → chroniony).
    pub image: String,
    /// Przodkowie (PID-y rodziców od najbliższego) w chwili sprawdzenia.
    pub ancestors: Vec<u32>,
}

/// Łańcuch przodków z mapy PID → PID rodzica (bez cykli, najwyżej [`MAX_ANCESTORS`]).
pub fn ancestors_of(pid: u32, parent_of: impl Fn(u32) -> Option<u32>) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let mut current = pid;
    while out.len() < MAX_ANCESTORS {
        match parent_of(current) {
            Some(p) if p != 0 && p != pid && !out.contains(&p) => {
                out.push(p);
                current = p;
            }
            _ => break,
        }
    }
    out
}

impl TargetGuard {
    /// Czy PID należy do zbioru korzeni chronionych drzew procesów (bieżący proces i `pids`) —
    /// każdy ich potomek jest chroniony.
    pub fn is_protected_root(&self, pid: u32) -> bool {
        pid != 0 && (pid == std::process::id() || self.pids.contains(&pid))
    }

    /// Czy proces jest chroniony sam albo jako potomek procesu chronionego.
    pub fn is_protected_link(&self, link: &ProcessLink) -> bool {
        link.role == LinkRole::UwpUnresolved
            || self.is_protected(link.pid, &link.image)
            || link.ancestors.iter().any(|a| self.is_protected_root(*a))
    }

    /// Okno (proces efektywny + procesy powiązane) — chronione, gdy którykolwiek proces jest
    /// chroniony (fail-closed). Ramka UWP jako proces efektywny = treść nierozpoznana.
    pub fn is_protected_window(&self, pid: u32, image: &str, links: &[ProcessLink]) -> bool {
        self.is_protected(pid, image)
            || image_file_name(image) == UWP_FRAME_HOST
            || links.iter().any(|l| self.is_protected_link(l))
    }

    /// Sprawdzenie okna przed akcją: `Err(ProtectedTarget)` dla okna chronionego.
    pub fn check_window(
        &self,
        pid: u32,
        image: &str,
        links: &[ProcessLink],
        what: &str,
    ) -> Result<(), GuiError> {
        self.check(pid, image, what)?;
        if let Some(l) = links.iter().find(|l| self.is_protected_link(l)) {
            let shown = match l.role {
                LinkRole::UwpUnresolved => "nierozpoznana aplikacja UWP".to_owned(),
                _ if l.image.is_empty() => "nieznany proces".to_owned(),
                _ => image_file_name(&l.image),
            };
            return Err(GuiError::ProtectedTarget(format!(
                "{what}: okno powiązane z procesem {shown} (PID {}) — okno Alfy (np. wyskakujące \
                 WebView2), Brokera albo proces nieznany; agentka nie steruje nim ani go nie odczytuje",
                l.pid
            )));
        }
        if image_file_name(image) == UWP_FRAME_HOST {
            return Err(GuiError::ProtectedTarget(format!(
                "{what}: ramka UWP bez rozpoznanej aplikacji — agentka jej nie steruje"
            )));
        }
        Ok(())
    }
}
