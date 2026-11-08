//! Akcje UI Automation przez wzorce i ich sprawdzenie przed wykonaniem (wspólne dla atrapy
//! i implementacji): wzorzec, element włączony, zakaz wpisywania w pole hasła, limit długości.

use serde::{Deserialize, Serialize};

use crate::gui::GuiError;
use crate::uia::{MAX_SET_VALUE_CHARS, UiaNode, UiaPattern};

/// Kierunek przewijania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    /// W górę.
    Up,
    /// W dół.
    Down,
    /// W lewo.
    Left,
    /// W prawo.
    Right,
}

/// Wielkość kroku przewijania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollAmount {
    /// Mały krok (linia).
    Small,
    /// Duży krok (strona).
    Large,
}

/// Akcja przez wzorzec UIA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum UiaAction {
    /// `InvokePattern.Invoke`.
    Invoke,
    /// `ValuePattern.SetValue`.
    SetValue {
        /// Nowa wartość.
        value: String,
    },
    /// `TogglePattern.Toggle`.
    Toggle,
    /// `ExpandCollapsePattern.Expand`.
    Expand,
    /// `ExpandCollapsePattern.Collapse`.
    Collapse,
    /// `SelectionItemPattern.Select`.
    Select,
    /// `ScrollPattern.Scroll`.
    Scroll {
        /// Kierunek.
        direction: ScrollDirection,
        /// Wielkość.
        amount: ScrollAmount,
    },
}

impl UiaAction {
    /// Wymagany wzorzec.
    pub fn pattern(&self) -> UiaPattern {
        match self {
            Self::Invoke => UiaPattern::Invoke,
            Self::SetValue { .. } => UiaPattern::Value,
            Self::Toggle => UiaPattern::Toggle,
            Self::Expand | Self::Collapse => UiaPattern::ExpandCollapse,
            Self::Select => UiaPattern::SelectionItem,
            Self::Scroll { .. } => UiaPattern::Scroll,
        }
    }

    /// Nazwa (zdarzenia, audyt — bez treści wartości).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Invoke => "invoke",
            Self::SetValue { .. } => "set_value",
            Self::Toggle => "toggle",
            Self::Expand => "expand",
            Self::Collapse => "collapse",
            Self::Select => "select",
            Self::Scroll { .. } => "scroll",
        }
    }

    /// Sprawdzenie przed akcją (wspólne dla implementacji): wzorzec, włączony element, zakaz
    /// wpisywania w pole hasła (agentka nie zna haseł — sekrety tylko przez `secrets.read`), limit.
    pub fn check(&self, node: &UiaNode) -> Result<(), GuiError> {
        if !node.enabled {
            return Err(GuiError::PatternUnsupported(format!(
                "element „{}” jest wyłączony",
                node.name
            )));
        }
        if !node.supports(self.pattern()) {
            return Err(GuiError::PatternUnsupported(format!(
                "„{}” ({}) nie obsługuje wzorca {:?}",
                node.name,
                node.role,
                self.pattern()
            )));
        }
        if let Self::SetValue { value } = self {
            if node.is_password {
                return Err(GuiError::Policy(
                    "pole hasła — agentka nie wpisuje haseł".into(),
                ));
            }
            if value.chars().count() > MAX_SET_VALUE_CHARS || value.contains('\0') {
                return Err(GuiError::Policy(format!(
                    "wartość dłuższa niż {MAX_SET_VALUE_CHARS} znaków albo z NUL"
                )));
            }
        }
        Ok(())
    }
}
