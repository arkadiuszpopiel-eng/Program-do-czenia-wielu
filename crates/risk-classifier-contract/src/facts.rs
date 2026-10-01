//! Fakty o akcji — wejście klasyfikatora. Wypełnia je Broker (zakres, taint, reguły Jądra)
//! na podstawie manifestu narzędzia i stanu sesji; klasyfikator nie czyta argumentów sam.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::{
    ActionClass, CommandOrigin, Destructiveness, KernelRule, Reversibility, ScopeRelation,
};

/// Fakty o jednej akcji.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ActionFacts {
    /// Narzędzie (np. `tools-fs.delete`).
    pub tool: String,
    /// Klasa akcji.
    pub class: ActionClass,
    /// Odwracalność z manifestu narzędzia.
    pub reversible: Reversibility,
    /// Relacja do zakresu sesji.
    pub scope: ScopeRelation,
    /// Host docelowy, jeśli akcja wysyła dane na zewnątrz.
    pub egress: Option<String>,
    /// Czy host jest na egress-allowliście (polityka Jądra).
    pub egress_allowlisted: bool,
    /// Destrukcyjność.
    pub destructive: Destructiveness,
    /// Liczba obiektów, których dotyczy akcja (1 = pojedyncza).
    pub bulk: u32,
    /// Czy akcja instaluje oprogramowanie.
    pub install: bool,
    /// Źródło polecenia.
    pub origin: CommandOrigin,
    /// Czy sesja widziała niezaufaną treść (`tainted`, PLAN §8.7).
    pub tainted: bool,
    /// Czy argumenty akcji pochodzą z niezaufanej treści.
    pub untrusted_input_in_args: bool,
    /// Czy przebieg ma dostęp do danych prywatnych (składnik A trifecty).
    pub touches_private_data: bool,
    /// Twarda reguła Jądra wykryta przez Brokera (ścieżki, procesy, polecenia, deny-listy).
    pub kernel_rule: Option<KernelRule>,
}

impl ActionFacts {
    /// Fakty minimalne: odwracalna, w zakresie, bez egressu, pojedyncza, zlecona tekstem.
    pub fn new(tool: impl Into<String>, class: ActionClass) -> Self {
        Self {
            tool: tool.into(),
            class,
            reversible: Reversibility::Yes,
            scope: ScopeRelation::InScope,
            egress: None,
            egress_allowlisted: false,
            destructive: Destructiveness::None,
            bulk: 1,
            install: false,
            origin: CommandOrigin::UserText,
            tainted: false,
            untrusted_input_in_args: false,
            touches_private_data: false,
            kernel_rule: None,
        }
    }

    /// Ustawia odwracalność.
    #[must_use]
    pub fn reversible(mut self, r: Reversibility) -> Self {
        self.reversible = r;
        self
    }

    /// Ustawia relację do zakresu.
    #[must_use]
    pub fn scope(mut self, s: ScopeRelation) -> Self {
        self.scope = s;
        self
    }

    /// Ustawia host egressu i przynależność do allowlisty.
    #[must_use]
    pub fn egress(mut self, host: impl Into<String>, allowlisted: bool) -> Self {
        self.egress = Some(host.into());
        self.egress_allowlisted = allowlisted;
        self
    }

    /// Ustawia destrukcyjność.
    #[must_use]
    pub fn destructive(mut self, d: Destructiveness) -> Self {
        self.destructive = d;
        self
    }

    /// Ustawia liczbę obiektów.
    #[must_use]
    pub fn bulk(mut self, n: u32) -> Self {
        self.bulk = n;
        self
    }

    /// Oznacza instalację oprogramowania.
    #[must_use]
    pub fn install(mut self) -> Self {
        self.install = true;
        self
    }

    /// Ustawia źródło polecenia.
    #[must_use]
    pub fn origin(mut self, o: CommandOrigin) -> Self {
        self.origin = o;
        self
    }

    /// Oznacza sesję jako `tainted`.
    #[must_use]
    pub fn tainted(mut self) -> Self {
        self.tainted = true;
        self
    }

    /// Oznacza argumenty jako pochodzące z niezaufanej treści.
    #[must_use]
    pub fn untrusted_args(mut self) -> Self {
        self.untrusted_input_in_args = true;
        self
    }

    /// Oznacza dostęp do danych prywatnych.
    #[must_use]
    pub fn private_data(mut self) -> Self {
        self.touches_private_data = true;
        self
    }

    /// Ustawia wykrytą regułę Jądra.
    #[must_use]
    pub fn kernel(mut self, rule: KernelRule) -> Self {
        self.kernel_rule = Some(rule);
        self
    }

    /// Czy akcja wysyła dane na zewnątrz.
    pub fn is_egress(&self) -> bool {
        self.egress.is_some() || self.class == ActionClass::Egress
    }

    /// Czy akcja zmienia stan (wszystko poza czystym odczytem).
    pub fn is_mutating(&self) -> bool {
        !matches!(self.class, ActionClass::Read | ActionClass::SecretsRead)
            || self.destructive != Destructiveness::None
            || self.install
            || self.is_egress()
    }

    /// Czy akcja wynika z niezaufanej treści (źródło albo argumenty).
    pub fn untrusted_driven(&self) -> bool {
        self.origin == CommandOrigin::UntrustedContent || self.untrusted_input_in_args
    }

    /// Czy akcja jest efektywnie nieodwracalna (`no` albo `scoped` poza zakresem snapshotu).
    pub fn effectively_irreversible(&self) -> bool {
        self.reversible == Reversibility::No
            || (self.reversible == Reversibility::Scoped && self.scope == ScopeRelation::Outside)
    }

    /// „Lethal trifecta”: dane prywatne + niezaufana treść + kanał wyjścia (THREAT_MODEL §5).
    pub fn trifecta(&self) -> bool {
        self.touches_private_data && (self.tainted || self.untrusted_driven()) && self.is_egress()
    }
}
