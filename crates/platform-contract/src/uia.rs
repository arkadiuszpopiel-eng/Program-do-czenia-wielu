//! Port UI Automation (F5 v1.5 / F6 v2, PLAN §7.1–7.3): odczyt drzewa elementów okna (nazwa,
//! rola, wartość, stan, prostokąt, AutomationId, wzorce), wyszukiwanie po kryteriach,
//! `TextPattern` tylko do odczytu, akcje wyłącznie przez wzorce (`Invoke`, `Value.SetValue`,
//! `Toggle`, `ExpandCollapse`, `SelectionItem`, `Scroll`) z limitami czasu.
//!
//! Niezmienniki (testowane na atrapie i implementacji): wartość pól haseł (`IsPassword`) nigdy
//! nie wychodzi z portu; okna i elementy procesów chronionych ([`crate::TargetGuard`]) nie są ani
//! czytane, ani sterowane; każda akcja zwraca odświeżony element (weryfikacja po akcji, F6-04).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::gui::{GuiError, ScreenRect};
pub use crate::uia_action::{ScrollAmount, ScrollDirection, UiaAction};
use crate::window::WindowId;

/// Domyślny limit pojedynczego wywołania UIA (ms).
pub const UIA_CALL_TIMEOUT_MS: u64 = 5_000;
/// Domyślny limit odczytu całego drzewa (ms).
pub const UIA_TREE_TIMEOUT_MS: u64 = 15_000;
/// Drzewo o tylu węzłach albo mniej uznajemy za „ubogie” (Electron/DirectX/Java) → trasa wizji (§7.1).
pub const SPARSE_TREE_NODES: usize = 5;
/// Maksymalna długość wartości w `SetValue` (znaki).
pub const MAX_SET_VALUE_CHARS: usize = 10_000;
/// Maksymalna długość identyfikatora wykonania elementu.
const MAX_RUNTIME_ID_LEN: usize = 16;

/// Odwołanie do elementu: okno + `RuntimeId` UIA. Postać tekstowa: `w<okno>:<a.b.c>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ElementRef {
    /// Okno najwyższego poziomu, w którym leży element.
    pub window: WindowId,
    /// `RuntimeId` (unikalny w czasie życia elementu).
    pub runtime_id: Vec<i32>,
}

impl ElementRef {
    /// Parsuje postać tekstową.
    pub fn parse(text: &str) -> Result<Self, GuiError> {
        let bad = || GuiError::Policy(format!("niepoprawne odwołanie do elementu `{text}`"));
        let rest = text.trim().strip_prefix('w').ok_or_else(bad)?;
        let (window, rid) = rest.split_once(':').ok_or_else(bad)?;
        let window = window.parse::<u64>().map_err(|_| bad())?;
        let runtime_id = rid
            .split('.')
            .map(|p| p.parse::<i32>().map_err(|_| bad()))
            .collect::<Result<Vec<_>, _>>()?;
        if runtime_id.is_empty() || runtime_id.len() > MAX_RUNTIME_ID_LEN {
            return Err(bad());
        }
        Ok(Self {
            window: WindowId(window),
            runtime_id,
        })
    }
}

impl fmt::Display for ElementRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rid: Vec<String> = self.runtime_id.iter().map(i32::to_string).collect();
        write!(f, "w{}:{}", self.window.0, rid.join("."))
    }
}

impl TryFrom<String> for ElementRef {
    type Error = GuiError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<ElementRef> for String {
    fn from(value: ElementRef) -> Self {
        value.to_string()
    }
}

/// Wzorce UIA istotne dla agentek.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiaPattern {
    /// `InvokePattern`.
    Invoke,
    /// `ValuePattern`.
    Value,
    /// `TogglePattern`.
    Toggle,
    /// `ExpandCollapsePattern`.
    ExpandCollapse,
    /// `SelectionItemPattern`.
    SelectionItem,
    /// `ScrollPattern`.
    Scroll,
    /// `TextPattern` (tylko odczyt).
    Text,
}

/// Stan przełącznika.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToggleState {
    /// Wyłączony.
    Off,
    /// Włączony.
    On,
    /// Nieokreślony.
    Indeterminate,
}

/// Stan rozwinięcia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpandState {
    /// Zwinięty.
    Collapsed,
    /// Rozwinięty.
    Expanded,
    /// Częściowo rozwinięty.
    PartiallyExpanded,
    /// Liść (nie da się rozwinąć).
    LeafNode,
}

/// Element drzewa (migawka).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiaNode {
    /// Odwołanie.
    pub element: ElementRef,
    /// Głębokość względem okna (0 = okno).
    pub depth: u16,
    /// PID procesu dostawcy elementu.
    pub pid: u32,
    /// Rola (`button`, `edit`, … — [`control_type_name`]).
    pub role: String,
    /// Nazwa (treść niezaufana).
    pub name: String,
    /// `AutomationId`.
    pub automation_id: String,
    /// Klasa.
    pub class_name: String,
    /// Wartość (`ValuePattern`); **zawsze `None` dla pól haseł**.
    pub value: Option<String>,
    /// `IsPassword`.
    pub is_password: bool,
    /// Czy włączony.
    pub enabled: bool,
    /// Czy poza ekranem.
    pub offscreen: bool,
    /// Czy ma fokus klawiatury.
    pub focused: bool,
    /// Stan przełącznika.
    pub toggle: Option<ToggleState>,
    /// Stan rozwinięcia.
    pub expand: Option<ExpandState>,
    /// Zaznaczenie (`SelectionItem`).
    pub selected: Option<bool>,
    /// Prostokąt na ekranie.
    pub rect: ScreenRect,
    /// Obsługiwane wzorce.
    pub patterns: Vec<UiaPattern>,
}

impl UiaNode {
    /// Usuwa wartość pola hasła (wołane przez każdą implementację przed zwróceniem węzła).
    #[must_use]
    pub fn redacted(mut self) -> Self {
        if self.is_password {
            self.value = None;
        }
        self
    }

    /// Czy obsługuje wzorzec.
    pub fn supports(&self, pattern: UiaPattern) -> bool {
        self.patterns.contains(&pattern)
    }
}

/// Opcje odczytu drzewa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeOptions {
    /// Maksymalna głębokość.
    pub max_depth: u16,
    /// Maksymalna liczba węzłów (reszta → `truncated`).
    pub max_nodes: usize,
    /// Czy dołączać elementy poza ekranem.
    pub include_offscreen: bool,
}

impl Default for TreeOptions {
    fn default() -> Self {
        Self {
            max_depth: 12,
            max_nodes: 400,
            include_offscreen: false,
        }
    }
}

/// Migawka drzewa okna (kolejność preorder, `depth` odtwarza strukturę).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiaTree {
    /// Okno.
    pub window: WindowId,
    /// Węzły.
    pub nodes: Vec<UiaNode>,
    /// Obcięto limitem węzłów lub głębokości.
    pub truncated: bool,
}

impl UiaTree {
    /// Drzewo „ubogie” — trasa UIA nie wystarczy, przełącz na wizję (§7.1, F6-03).
    pub fn is_sparse(&self) -> bool {
        self.nodes.len() <= SPARSE_TREE_NODES
    }
}

/// Kryteria wyszukiwania (wszystkie podane muszą pasować; napisy bez wielkości liter).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiaQuery {
    /// Nazwa dokładnie.
    pub name: Option<String>,
    /// Fragment nazwy.
    pub name_contains: Option<String>,
    /// Rola (`button`, `edit`…).
    pub role: Option<String>,
    /// `AutomationId` dokładnie.
    pub automation_id: Option<String>,
    /// Klasa dokładnie.
    pub class_name: Option<String>,
    /// Maksymalna liczba wyników.
    pub max_results: usize,
}

impl UiaQuery {
    /// Czy nie podano żadnego kryterium.
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.name_contains.is_none()
            && self.role.is_none()
            && self.automation_id.is_none()
            && self.class_name.is_none()
    }

    /// Czy węzeł pasuje (ta sama logika w każdej implementacji).
    pub fn matches(&self, node: &UiaNode) -> bool {
        let eq = |want: &Option<String>, have: &str| {
            want.as_ref()
                .is_none_or(|w| w.trim().to_lowercase() == have.trim().to_lowercase())
        };
        eq(&self.name, &node.name)
            && eq(&self.role, &node.role)
            && eq(&self.automation_id, &node.automation_id)
            && eq(&self.class_name, &node.class_name)
            && self
                .name_contains
                .as_ref()
                .is_none_or(|c| node.name.to_lowercase().contains(&c.trim().to_lowercase()))
    }
}

/// Tekst z `TextPattern` (tylko odczyt).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiaText {
    /// Tekst (≤ limit).
    pub text: String,
    /// Obcięto.
    pub truncated: bool,
}

/// Port UI Automation. Wszystkie metody blokują wywołującego co najwyżej do limitu czasu
/// implementacji ([`UIA_CALL_TIMEOUT_MS`], drzewo [`UIA_TREE_TIMEOUT_MS`]) → `GuiError::Timeout`.
pub trait UiaPort: Send + Sync {
    /// Drzewo okna (odmowa dla okna chronionego).
    fn tree(&self, window: WindowId, options: &TreeOptions) -> Result<UiaTree, GuiError>;
    /// Elementy pasujące do kryteriów (pusty zestaw kryteriów = błąd zasad).
    fn find(&self, window: WindowId, query: &UiaQuery) -> Result<Vec<UiaNode>, GuiError>;
    /// Bieżący stan elementu (weryfikacja po akcji).
    fn element(&self, element: &ElementRef) -> Result<UiaNode, GuiError>;
    /// Tekst dokumentu z `TextPattern` (odmowa dla pola hasła).
    fn read_text(&self, element: &ElementRef, max_chars: usize) -> Result<UiaText, GuiError>;
    /// Akcja przez wzorzec; zwraca odświeżony element.
    fn act(&self, element: &ElementRef, action: &UiaAction) -> Result<UiaNode, GuiError>;
    /// Prostokąty pól haseł w oknie (maskowanie zrzutów).
    fn password_rects(&self, window: WindowId) -> Result<Vec<ScreenRect>, GuiError>;
}

/// Nazwa roli dla `UIA_*ControlTypeId` (50000–50040); nieznane → `custom`.
pub fn control_type_name(id: i32) -> &'static str {
    const NAMES: [&str; 41] = [
        "button",
        "calendar",
        "check_box",
        "combo_box",
        "edit",
        "hyperlink",
        "image",
        "list_item",
        "list",
        "menu",
        "menu_bar",
        "menu_item",
        "progress_bar",
        "radio_button",
        "scroll_bar",
        "slider",
        "spinner",
        "status_bar",
        "tab",
        "tab_item",
        "text",
        "tool_bar",
        "tool_tip",
        "tree",
        "tree_item",
        "custom",
        "group",
        "thumb",
        "data_grid",
        "data_item",
        "document",
        "split_button",
        "window",
        "pane",
        "header",
        "header_item",
        "table",
        "title_bar",
        "separator",
        "semantic_zoom",
        "app_bar",
    ];
    usize::try_from(id - 50_000)
        .ok()
        .and_then(|i| NAMES.get(i).copied())
        .unwrap_or("custom")
}

#[cfg(test)]
#[path = "uia_tests.rs"]
mod tests;
