//! Narzędzia systemowe i sieciowe agentek (F6): `tools-system` nad `SysPort` (Windows: `WinSys`
//! ze strażnikiem celów — bieżący proces i jego drzewo, katalogi Alfy, lista bazowa Brokera,
//! Broker-UI i watchdoga) oraz portami odczytu (zasilanie, monitory, audio) i `tools-net` nad
//! klientem HTTPS z `lib-netguard` z kwarantanną pobrań `DiskDownloads`
//! (`<katalog roboczy sesji>\Kwarantanna`, zapasowo `%USERPROFILE%\Alfa\Kwarantanna\<sesja>`).
//! Wyszukiwarka — bez dostawcy (`net_search` nieobecne w rejestrze).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use platform_apps_contract::{DownloadStore, SysPort};
use platform_contract::{DesktopPort, HardwarePort, PowerPort, TargetGuard};
use tools_common_contract::{Tool, Toolset};
use tools_net_contract::{HttpPort, NetToolsConfig, SearchPort};
use tools_net_impl::{NetTools, NetToolsDeps, NoHttp, ReqwestHttp};
use tools_system_contract::SystemToolsConfig;
use tools_system_impl::{SystemTools, SystemToolsDeps};

use crate::toolset::ToolsDeps;

/// Porty narzędzi systemowych i sieciowych.
#[derive(Clone)]
pub struct SysNetDeps {
    /// Procesy, usługi, zdarzenia, zmienne.
    pub sys: Arc<dyn SysPort>,
    /// Zasilanie.
    pub power: Option<Arc<dyn PowerPort>>,
    /// Monitory.
    pub desktop: Option<Arc<dyn DesktopPort>>,
    /// Urządzenia audio.
    pub hardware: Option<Arc<dyn HardwarePort>>,
    /// Klient HTTPS (bez przekierowań, resolver tylko z adresami publicznymi).
    pub http: Arc<dyn HttpPort>,
    /// Wyszukiwarka (`None` — bez `net_search`).
    pub search: Option<Arc<dyn SearchPort>>,
    /// Kwarantanna pobrań.
    pub downloads: Arc<dyn DownloadStore>,
    /// Korzeń kwarantanny sesji bez katalogu roboczego (`None` — `%USERPROFILE%\Alfa\Kwarantanna`
    /// ze środowiska ścieżek narzędzi).
    pub quarantine_root: Option<PathBuf>,
}

impl std::fmt::Debug for SysNetDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SysNetDeps")
            .field("quarantine_root", &self.quarantine_root)
            .finish_non_exhaustive()
    }
}

/// Strażnik celów: lista bazowa (Alfa, Broker, Broker-UI, watchdog, helper) + bieżący proces
/// (jego potomkowie — przez łańcuch przodków liczony przy każdym wywołaniu) + katalogi Alfy.
pub fn sysnet_guard(local: &Path) -> TargetGuard {
    let mut dirs = vec![local.to_string_lossy().into_owned()];
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        dirs.push(dir.to_string_lossy().into_owned());
    }
    TargetGuard::baseline()
        .with_pids([std::process::id()])
        .with_image_dirs(dirs)
}

impl SysNetDeps {
    /// Porty systemowe nad katalogiem danych Alfy (`%LOCALAPPDATA%\Alfa`).
    pub fn system(local: &Path) -> Self {
        let http: Arc<dyn HttpPort> = match ReqwestHttp::system() {
            Ok(c) => Arc::new(c),
            Err(e) => Arc::new(NoHttp(e)),
        };
        Self {
            sys: Arc::new(platform_windows_sys_impl::WinSys::new(sysnet_guard(local))),
            power: Some(Arc::new(platform_windows_sys_impl::WinSignals::default())),
            desktop: Some(Arc::new(platform_windows_gui_impl::WinGui::default())),
            hardware: Some(Arc::new(platform_windows_impl::WinHardware)),
            http,
            search: None,
            downloads: Arc::new(platform_windows_sys_impl::DiskDownloads),
            quarantine_root: None,
        }
    }
}

/// Zestawy `tools-system` i `tools-net` nad Brokerem i magistralą z `deps`.
pub(crate) fn sysnet_tools(deps: &ToolsDeps, s: &SysNetDeps) -> (Vec<Arc<dyn Tool>>, SystemTools) {
    let system = SystemTools::new(SystemToolsDeps {
        sys: s.sys.clone(),
        power: s.power.clone(),
        desktop: s.desktop.clone(),
        hardware: s.hardware.clone(),
        broker: deps.broker.clone(),
        config: SystemToolsConfig::default(),
        bus: deps.bus.clone(),
    });
    let root = s.quarantine_root.clone().or_else(|| {
        deps.env
            .get("USERPROFILE")
            .map(|p| Path::new(p).join("Alfa").join("Kwarantanna"))
    });
    let net = NetTools::new(NetToolsDeps {
        http: s.http.clone(),
        search: s.search.clone(),
        downloads: s.downloads.clone(),
        quarantine_root: root,
        broker: deps.broker.clone(),
        deny: deps.deny.clone(),
        env: deps.env.clone(),
        config: NetToolsConfig::default(),
        bus: deps.bus.clone(),
    });
    let mut tools = system.tools();
    tools.extend(net.tools());
    (tools, system)
}
