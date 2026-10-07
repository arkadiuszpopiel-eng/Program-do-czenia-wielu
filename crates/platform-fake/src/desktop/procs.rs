//! Procesy wirtualnego pulpitu (przegląd bezpieczeństwa #2, P2-01): tabela PID → (obraz, rodzic),
//! okna w procesach już istniejących (np. wyskakujące okno WebView2 Alfy — `msedgewebview2.exe`,
//! potomek procesu Alfy), okna-własności i ramki UWP (`ApplicationFrameHost.exe` z treścią
//! w procesie aplikacji). Ochrona okna jest liczona **w chwili każdego sprawdzenia** z procesów
//! powiązanych i ich przodków — tak jak w implementacji Windows (`TargetGuard::check_window`).

use platform_contract::{
    DesktopWindow, LinkRole, ProcessLink, ScreenRect, TargetGuard, UWP_FRAME_CLASS, WindowId,
    WindowState, ancestors_of,
};

use super::{State, Win};

/// Obraz hosta ramek UWP w atrapie.
pub const FAKE_FRAME_HOST: &str = r"C:\Windows\System32\ApplicationFrameHost.exe";

/// Okno do dodania.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeWindow {
    /// Tytuł.
    pub title: String,
    /// Obraz procesu (ignorowany, gdy okno należy do istniejącego procesu — `process`).
    pub image: String,
    /// Prostokąt.
    pub rect: ScreenRect,
    /// Proces podniesiony.
    pub elevated: bool,
    /// Kolor wypełnienia na zrzucie.
    pub color: [u8; 4],
    /// Okno chronione przed przechwyceniem (czarna klatka).
    pub capture_blocked: bool,
    /// Okno istniejącego procesu ([`super::FakeDesktop::add_process`]) zamiast nowego.
    pub process: Option<u32>,
    /// Rodzic nowego procesu okna (np. PID Alfy dla procesu WebView2).
    pub parent: Option<u32>,
    /// Okno-właściciel (dialog, menu, lista rozwijana).
    pub owner: Option<WindowId>,
    /// Ramka UWP: obraz aplikacji z okna `CoreWindow` (pusty = treść nierozpoznana).
    pub uwp_content: Option<String>,
}

impl FakeWindow {
    /// Okno o tytule, obrazie i prostokącie (kolor szary), w nowym procesie bez rodzica.
    pub fn new(title: &str, image: &str, rect: ScreenRect) -> Self {
        Self {
            title: title.into(),
            image: image.into(),
            rect,
            elevated: false,
            color: [200, 200, 200, 255],
            capture_blocked: false,
            process: None,
            parent: None,
            owner: None,
            uwp_content: None,
        }
    }

    /// Ramka UWP z treścią aplikacji `app_image` (pusty = nierozpoznana, np. zawieszona).
    pub fn uwp(title: &str, app_image: &str, rect: ScreenRect) -> Self {
        Self {
            uwp_content: Some(app_image.into()),
            ..Self::new(title, FAKE_FRAME_HOST, rect)
        }
    }

    /// Okno w istniejącym procesie.
    #[must_use]
    pub fn in_process(mut self, pid: u32) -> Self {
        self.process = Some(pid);
        self
    }

    /// Nowy proces okna jest dzieckiem `parent`.
    #[must_use]
    pub fn child_of(mut self, parent: u32) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Okno-własność okna `owner`.
    #[must_use]
    pub fn owned_by(mut self, owner: WindowId) -> Self {
        self.owner = Some(owner);
        self
    }
}

/// Proces atrapy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FakeProc {
    pub(super) image: String,
    pub(super) parent: Option<u32>,
}

impl State {
    /// Dodaje okno na wierzch (z fokusem, jeśli `focus`). Proces okna: istniejący (`process`)
    /// albo nowy (z rodzicem `parent`); ramka UWP dostaje proces hosta i proces treści.
    pub(super) fn add_window(&mut self, spec: FakeWindow, focus: bool) -> WindowId {
        let s = self;
        let id = WindowId(s.next_id);
        s.next_id += 1;
        let (pid, image, frame) = match (&spec.uwp_content, spec.process) {
            (Some(app), _) => {
                let frame = s.spawn(&spec.image, spec.parent);
                if app.is_empty() {
                    (frame, spec.image.clone(), Some(frame))
                } else {
                    (s.spawn(app, None), app.clone(), Some(frame))
                }
            }
            (None, Some(pid)) => {
                let image = s.procs.get(&pid).map(|p| p.image.clone());
                let image = image.unwrap_or_else(|| spec.image.clone());
                (pid, image, None)
            }
            (None, None) => (s.spawn(&spec.image, spec.parent), spec.image.clone(), None),
        };
        let info = DesktopWindow {
            id,
            title: spec.title,
            class_name: if frame.is_some() {
                UWP_FRAME_CLASS.into()
            } else {
                "FakeWindow".into()
            },
            pid,
            protected: false,
            image,
            rect: spec.rect,
            monitor: 0,
            dpi: 96,
            state: WindowState::Normal,
            focused: false,
            z_order: 0,
            elevated: spec.elevated,
        };
        let current = s.windows.iter().find(|w| w.info.focused).map(|w| w.info.id);
        s.windows.insert(
            0,
            Win {
                info,
                color: spec.color,
                capture_blocked: spec.capture_blocked,
                restore: spec.rect,
                elements: Vec::new(),
                typed: Vec::new(),
                owner: spec.owner,
                frame,
            },
        );
        s.renumber(if focus { Some(id) } else { current });
        id
    }

    /// Nowy proces (PID z puli atrapy).
    pub(super) fn spawn(&mut self, image: &str, parent: Option<u32>) -> u32 {
        let pid = self.next_pid;
        self.next_pid += 1;
        self.procs.insert(
            pid,
            FakeProc {
                image: image.into(),
                parent,
            },
        );
        pid
    }

    fn link(&self, role: LinkRole, pid: u32) -> ProcessLink {
        ProcessLink {
            role,
            pid,
            image: self
                .procs
                .get(&pid)
                .map(|p| p.image.clone())
                .unwrap_or_default(),
            ancestors: ancestors_of(pid, |p| self.procs.get(&p).and_then(|x| x.parent)),
        }
    }

    /// Procesy powiązane z oknem (stan bieżący): proces okna, ramka UWP, łańcuch właścicieli.
    pub(super) fn links(&self, w: &Win) -> Vec<ProcessLink> {
        let mut out = Vec::new();
        let mut current = Some(w);
        let mut role = LinkRole::Root;
        for _ in 0..16 {
            let Some(win) = current else {
                break;
            };
            match win.frame {
                Some(frame) => {
                    out.push(self.link(role, frame));
                    if win.info.pid == frame {
                        out.push(ProcessLink {
                            role: LinkRole::UwpUnresolved,
                            ..ProcessLink::default()
                        });
                    } else {
                        out.push(self.link(LinkRole::UwpContent, win.info.pid));
                    }
                }
                None => out.push(self.link(role, win.info.pid)),
            }
            role = LinkRole::Owner;
            current = win.owner.and_then(|o| self.win(o).ok());
        }
        out
    }

    /// Opis okna z ochroną policzoną teraz.
    pub(super) fn info(&self, w: &Win, guard: &TargetGuard) -> DesktopWindow {
        let mut info = w.info.clone();
        info.protected = guard.is_protected_window(info.pid, &info.image, &self.links(w));
        info
    }
}
