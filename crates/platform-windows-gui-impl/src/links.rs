//! Procesy powiązane z oknem-celem (przegląd bezpieczeństwa #2, P2-01): proces okna najwyższego
//! poziomu (`GA_ROOT`), samego okna, łańcucha okien-własności (`GW_OWNER`, `GA_ROOTOWNER`) i —
//! dla ramek UWP (`ApplicationFrameWindow` z `ApplicationFrameHost.exe`) — proces aplikacji
//! z okna potomnego `Windows.UI.Core.CoreWindow`. Każdy z łańcuchem przodków z migawki Toolhelp32
//! **wziętej przy tym sprawdzeniu** (drzewo procesów Alfy liczone przy każdej akcji: proces
//! WebView2 odtworzony po awarii renderera też jest potomkiem Alfy, więc chroniony).
//!
//! Fail-closed: brak migawki = obraz nieznany (chroniony); ramka UWP bez rozpoznanej treści =
//! `UwpUnresolved` (chroniona); obraz efektywny okna UWP = aplikacja, nie host ramek (deny-lista
//! dostawców i „zawsze zezwalaj” działają na właściwej aplikacji).

#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::mem::size_of;

use platform_contract::{
    GuiError, LinkRole, ProcessLink, TargetGuard, TargetWindow, UWP_CORE_CLASS, UWP_FRAME_CLASS,
    ancestors_of,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GA_ROOTOWNER, GW_OWNER, GetAncestor, GetClassNameW, GetWindow,
};
use windows::core::{HSTRING, PCWSTR};

use crate::win::{
    OwnedHandle, from_wide, id_of, process_elevated, process_image, root_of, window_pid,
};

/// Najdłuższy uwzględniany łańcuch okien-własności.
const MAX_OWNERS: usize = 16;

/// Migawka drzewa procesów (PID → PID rodzica).
pub(crate) struct ProcessTree {
    parents: Option<BTreeMap<u32, u32>>,
}

impl ProcessTree {
    /// Migawka Toolhelp32 w tej chwili (błąd = brak migawki → procesy nieznane, chronione).
    pub(crate) fn snapshot() -> Self {
        // SAFETY: migawka procesów; uchwyt przejmuje `OwnedHandle`.
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        let Some(snapshot) = raw.ok().and_then(OwnedHandle::new) else {
            return Self { parents: None };
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(0),
            ..Default::default()
        };
        let mut parents = BTreeMap::new();
        // SAFETY: `entry.dwSize` ustawione zgodnie z wymogiem API; migawka ważna.
        let mut more = unsafe { Process32FirstW(snapshot.raw(), &raw mut entry) }.is_ok();
        while more {
            parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            // SAFETY: jw.
            more = unsafe { Process32NextW(snapshot.raw(), &raw mut entry) }.is_ok();
        }
        Self {
            parents: Some(parents),
        }
    }

    /// Proces z obrazem i przodkami.
    pub(crate) fn link(&self, role: LinkRole, pid: u32) -> ProcessLink {
        match &self.parents {
            Some(parents) => ProcessLink {
                role,
                pid,
                image: process_image(pid),
                ancestors: ancestors_of(pid, |p| parents.get(&p).copied()),
            },
            None => ProcessLink {
                role,
                pid,
                image: String::new(),
                ancestors: Vec::new(),
            },
        }
    }
}

/// Czy proces (z przodkami) jest chroniony.
pub(crate) fn pid_protected(guard: &TargetGuard, tree: &ProcessTree, pid: u32) -> bool {
    guard.is_protected_link(&tree.link(LinkRole::Window, pid))
}

/// Sprawdzenie procesu (z przodkami) przed akcją.
pub(crate) fn check_pid(
    guard: &TargetGuard,
    tree: &ProcessTree,
    pid: u32,
    what: &str,
) -> Result<(), GuiError> {
    let link = tree.link(LinkRole::Window, pid);
    guard.check_window(pid, &link.image, std::slice::from_ref(&link), what)
}

/// Klasa okna.
pub(crate) fn class_of(hwnd: HWND) -> String {
    let mut class = vec![0u16; 256];
    // SAFETY: bufor o znanym rozmiarze.
    let n = usize::try_from(unsafe { GetClassNameW(hwnd, &mut class) }).unwrap_or(0);
    from_wide(&class[..n.min(class.len())])
}

/// PID procesu treści ramki UWP (okno potomne `CoreWindow`).
fn uwp_content_pid(frame: HWND, frame_pid: u32) -> Option<u32> {
    let class = HSTRING::from(UWP_CORE_CLASS);
    // SAFETY: wyszukanie okna potomnego ramki po klasie (bez tytułu).
    let core = unsafe { FindWindowExW(Some(frame), None, &class, PCWSTR::null()) }.ok()?;
    let pid = window_pid(core);
    (pid != 0 && pid != frame_pid).then_some(pid)
}

/// Okno-cel: okno najwyższego poziomu, proces efektywny (aplikacja UWP zamiast hosta ramek)
/// i procesy powiązane z przodkami.
pub(crate) struct WindowTarget {
    /// Okno najwyższego poziomu.
    pub(crate) root: HWND,
    /// Proces efektywny.
    pub(crate) pid: u32,
    /// Obraz procesu efektywnego.
    pub(crate) image: String,
    /// Procesy powiązane.
    pub(crate) links: Vec<ProcessLink>,
}

impl WindowTarget {
    /// Ochrona okna strażnikiem.
    pub(crate) fn protected(&self, guard: &TargetGuard) -> bool {
        guard.is_protected_window(self.pid, &self.image, &self.links)
    }

    /// Sprawdzenie strażnikiem przed akcją.
    pub(crate) fn check(&self, guard: &TargetGuard, what: &str) -> Result<(), GuiError> {
        guard.check_window(self.pid, &self.image, &self.links, what)
    }
}

/// Okno-cel dla dowolnego okna (także potomnego albo wyskakującego).
pub(crate) fn window_target(hwnd: HWND, tree: &ProcessTree) -> WindowTarget {
    let root = root_of(hwnd);
    let root_pid = window_pid(root);
    let root_link = tree.link(LinkRole::Root, root_pid);
    let (mut pid, mut image) = (root_pid, root_link.image.clone());
    let mut links = vec![root_link];
    let add = |links: &mut Vec<ProcessLink>, role: LinkRole, p: u32| {
        if p != 0 && !links.iter().any(|l| l.pid == p) {
            links.push(tree.link(role, p));
        }
    };
    add(&mut links, LinkRole::Window, window_pid(hwnd));
    let mut owner = root;
    for _ in 0..MAX_OWNERS {
        // SAFETY: zapytanie o właściciela okna; brak = błąd.
        match unsafe { GetWindow(owner, GW_OWNER) } {
            Ok(o) if !o.is_invalid() => {
                add(&mut links, LinkRole::Owner, window_pid(o));
                owner = o;
            }
            _ => break,
        }
    }
    // SAFETY: zapytanie o korzeń łańcucha rodziców i właścicieli.
    let root_owner = unsafe { GetAncestor(hwnd, GA_ROOTOWNER) };
    if !root_owner.is_invalid() {
        add(&mut links, LinkRole::Owner, window_pid(root_owner));
    }
    if class_of(root) == UWP_FRAME_CLASS {
        match uwp_content_pid(root, root_pid) {
            Some(content) => {
                let link = tree.link(LinkRole::UwpContent, content);
                (pid, image) = (content, link.image.clone());
                links.retain(|l| l.pid != content);
                links.push(link);
            }
            None => links.push(ProcessLink {
                role: LinkRole::UwpUnresolved,
                ..ProcessLink::default()
            }),
        }
    }
    WindowTarget {
        root,
        pid,
        image,
        links,
    }
}

/// Cel wejścia dla okna (właściciel efektywny, obraz, podniesienie, procesy powiązane).
pub(crate) fn target_of(hwnd: HWND) -> Option<TargetWindow> {
    if hwnd.is_invalid() {
        return None;
    }
    let t = window_target(hwnd, &ProcessTree::snapshot());
    let root_pid = window_pid(t.root);
    Some(TargetWindow {
        id: id_of(t.root),
        pid: t.pid,
        elevated: process_elevated(t.pid) || process_elevated(root_pid),
        image: t.image,
        links: t.links,
    })
}
