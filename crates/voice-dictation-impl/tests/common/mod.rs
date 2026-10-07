//! Wirtualny pulpit testów: `platform-fake::FakeDesktop` + odtworzenie tekstu pola z faktycznie
//! dostarczonych zdarzeń (Unicode, Enter, Backspace — atrapa nie interpretuje Backspace).

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use platform_contract::{DesktopPort, ScreenRect, WindowId};
use platform_fake::{FakeDesktop, FakeElement, FakeWindow, GuiRecordKind};
use voice_dictation_contract::DictationCfg;
use voice_dictation_contract::contract_tests::DesktopDriver;
use voice_dictation_impl::{DictationPorts, DictationService};

pub struct Desk(pub Arc<FakeDesktop>);

fn rect() -> ScreenRect {
    ScreenRect::from_xywh(100, 100, 800, 600)
}

fn field(f: &str, key: &str) -> Option<u32> {
    let start = f.find(key)? + key.len();
    f[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

impl Desk {
    pub fn new() -> Self {
        Self(Arc::new(FakeDesktop::new()))
    }

    pub fn service(&self) -> DictationService {
        let ports = DictationPorts {
            desktop: self.0.clone(),
            uia: self.0.clone(),
            input: self.0.clone(),
        };
        DictationService::new(ports, DictationCfg::default())
    }

    /// Tekst pola po kolei zdarzeń (Backspace usuwa znak).
    pub fn replay(&self, window: WindowId) -> String {
        let mut units: Vec<u16> = Vec::new();
        for r in self.0.records().into_iter().filter(|r| r.window == window) {
            let GuiRecordKind::Input(e) = r.kind else {
                continue;
            };
            if e.contains("up: true") {
                continue;
            }
            if e.starts_with("Unicode") {
                units.extend(field(&e, "unit: ").and_then(|u| u16::try_from(u).ok()));
            } else if e.starts_with("Key") {
                match field(&e, "vk: ") {
                    Some(0x0D) => units.push(0x0A),
                    Some(0x08) => {
                        units.pop();
                    }
                    _ => {}
                }
            }
        }
        String::from_utf16_lossy(&units)
    }
}

impl DesktopDriver for Desk {
    fn open_app(&self, image: &str) -> u64 {
        let w = self.0.add_window(
            FakeWindow::new(image, &format!(r"C:\Apps\{image}"), rect()),
            true,
        );
        self.0
            .add_element(w, FakeElement::new("Edytor", "edit", rect()));
        w.0
    }

    fn open_password_app(&self) -> u64 {
        let w = self.0.add_window(
            FakeWindow::new("Logowanie", r"C:\Apps\bank.exe", rect()),
            true,
        );
        self.0
            .add_element(w, FakeElement::new("Hasło", "edit", rect()).password());
        w.0
    }

    fn open_elevated_app(&self) -> u64 {
        let mut spec = FakeWindow::new("Admin", r"C:\Windows\regedit.exe", rect());
        spec.elevated = true;
        self.0.add_window(spec, true).0
    }

    fn focus(&self, window: u64) {
        DesktopPort::focus(self.0.as_ref(), WindowId(window)).unwrap();
    }

    fn text(&self, window: u64) -> String {
        self.replay(WindowId(window))
    }
}
