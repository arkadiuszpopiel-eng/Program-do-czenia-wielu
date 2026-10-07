//! Spójność zrzutu z listą okien (przegląd bezpieczeństwa #2, P2-02 — TOCTOU maskowania): lista
//! okien jest brana przed przechwyceniem klatki, więc okno chronione albo z deny-listy (Broker-UI,
//! szybkie okno Alfy, menedżer haseł), które pojawi się lub przesunie między wyliczeniem
//! a zrzutem, nie byłoby zamaskowane. Dlatego po klatce okna są wyliczane **ponownie**:
//!
//! - zbiór istotny dla maskowania bez zmian → klatka przyjęta (maska z drugiego wyliczenia);
//! - zmiana → klatka odrzucona i ponowiona (najwyżej [`CAPTURE_ATTEMPTS`] razy);
//! - po wyczerpaniu prób → maska z **sumy** obu wyliczeń (stara i nowa pozycja okien chronionych
//!   i maskowanych) oraz okna, które zmieniły położenie lub stan, maskowane w całości w obu
//!   położeniach (pola haseł mogły być w dowolnym z nich) — fail-closed.
//!
//! Testy: `platform-fake/tests/review_contract.rs` (limit rozmiaru crate'a kontraktu).

use crate::capture::{MaskReason, MaskedArea};
use crate::desktop::{DesktopWindow, WindowState};
use crate::gui::ScreenRect;
use crate::window::WindowId;

/// Ile razy przechwycić klatkę, zanim maska obejmie sumę wyliczeń.
pub const CAPTURE_ATTEMPTS: usize = 3;

type Key = (WindowId, ScreenRect, WindowState, bool, String);

fn relevant(source: &ScreenRect, windows: &[DesktopWindow]) -> Vec<Key> {
    let mut out: Vec<Key> = windows
        .iter()
        .filter(|w| w.state != WindowState::Minimized && source.intersect(&w.rect).is_some())
        .map(|w| (w.id, w.rect, w.state, w.protected, w.image.to_lowercase()))
        .collect();
    out.sort_by_key(|k| (k.0.0, k.1.left, k.1.top, k.1.right, k.1.bottom));
    out
}

/// Czy wyliczenia przed i po klatce opisują ten sam stan istotny dla maskowania w obszarze
/// (okna, ich prostokąty, stan, ochrona i obraz).
pub fn capture_set_stable(
    source: &ScreenRect,
    before: &[DesktopWindow],
    after: &[DesktopWindow],
) -> bool {
    relevant(source, before) == relevant(source, after)
}

/// Lista okien do maskowania po wyczerpaniu prób: okna z `after` oraz okna z `before` w starej
/// pozycji/stanie (okno chronione, które zniknęło albo się przesunęło, nadal jest maskowane).
pub fn union_for_mask(before: &[DesktopWindow], after: &[DesktopWindow]) -> Vec<DesktopWindow> {
    let mut out = after.to_vec();
    for w in before {
        let same = after
            .iter()
            .any(|a| a.id == w.id && a.rect == w.rect && a.state == w.state);
        if !same {
            out.push(w.clone());
        }
    }
    out
}

/// Okna niechronione, które zmieniły położenie, stan albo pojawiły się/zniknęły między
/// wyliczeniami — maskowane w całości (w obu położeniach), bo ich pól haseł nie da się
/// przypisać do klatki.
pub fn unstable_masks(
    source: &ScreenRect,
    before: &[DesktopWindow],
    after: &[DesktopWindow],
) -> Vec<MaskedArea> {
    let changed = |list: &[DesktopWindow], other: &[DesktopWindow]| -> Vec<ScreenRect> {
        list.iter()
            .filter(|w| w.state != WindowState::Minimized)
            .filter(|w| {
                !other
                    .iter()
                    .any(|o| o.id == w.id && o.rect == w.rect && o.state == w.state)
            })
            .filter_map(|w| source.intersect(&w.rect))
            .collect()
    };
    changed(before, after)
        .into_iter()
        .chain(changed(after, before))
        .map(|rect| MaskedArea {
            rect,
            reason: MaskReason::Unverified,
        })
        .collect()
}
