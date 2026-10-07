//! Pole z fokusem i pola haseł przez UIA (tylko odczyt metadanych — `IsPassword`, nigdy wartości).
//!
//! Zasada **fail-closed**: wpisywanie jest dozwolone, gdy element z fokusem znaleziono i nie jest
//! polem hasła, albo gdy fokusu nie da się ustalić, ale w oknie nie ma żadnego pola hasła. Błąd
//! albo zawieszenie UIA (limit czasu) = traktujemy jak pole hasła (odmowa).

use platform_contract::{TreeOptions, UiaNode, UiaPort, WindowId};

/// Opcje odczytu drzewa przy szukaniu fokusu (głęboko, ale z limitem węzłów).
const FOCUS_TREE: TreeOptions = TreeOptions {
    max_depth: 32,
    max_nodes: 2_000,
    include_offscreen: false,
};

/// Element z fokusem klawiatury w oknie (ostatni w kolejności preorder — najgłębszy).
pub fn focused_node(uia: &dyn UiaPort, window: WindowId) -> Result<Option<UiaNode>, String> {
    let tree = uia.tree(window, &FOCUS_TREE).map_err(|e| e.to_string())?;
    Ok(tree.nodes.into_iter().rfind(|n| n.focused))
}

/// Czy wpisanie do okna grozi wpisaniem w pole hasła (`true` = odmowa).
pub fn password_risk(uia: &dyn UiaPort, window: WindowId) -> bool {
    match focused_node(uia, window) {
        Ok(Some(node)) => node.is_password,
        Ok(None) => match uia.password_rects(window) {
            Ok(rects) => !rects.is_empty(),
            Err(_) => true,
        },
        Err(_) => true,
    }
}
