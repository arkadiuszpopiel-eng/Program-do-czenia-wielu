//! Odpytywanie pracy asynchronicznej z wątku przetwarzania bez blokowania: każda wolna operacja
//! (STT, LLM, TTS, magistrala) jest przyszłością trzymaną w potoku i odpytywaną raz na krok
//! kontekstem zadania potoku (prawdziwe I/O budzi zadanie; atrapy z zegarem wirtualnym są po
//! prostu odpytywane w kolejnych krokach).

use std::pin::Pin;
use std::task::{Context, Poll};

/// Przyszłość trzymana między krokami.
pub(crate) type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Odpytuje przyszłość w slocie; gotowy wynik opróżnia slot.
pub(crate) fn poll_slot<T>(slot: &mut Option<BoxFut<T>>, cx: &mut Context<'_>) -> Option<T> {
    let fut = slot.as_mut()?;
    match fut.as_mut().poll(cx) {
        Poll::Ready(v) => {
            *slot = None;
            Some(v)
        }
        Poll::Pending => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::Waker;

    #[test]
    fn slot_is_emptied_when_ready() {
        let mut cx = Context::from_waker(Waker::noop());
        let mut slot: Option<BoxFut<u8>> = Some(Box::pin(async { 7 }));
        assert_eq!(poll_slot(&mut slot, &mut cx), Some(7));
        assert!(slot.is_none());
        assert_eq!(poll_slot(&mut slot, &mut cx), None);
        let mut pending: Option<BoxFut<u8>> = Some(Box::pin(std::future::pending()));
        assert_eq!(poll_slot(&mut pending, &mut cx), None);
        assert!(pending.is_some());
    }
}
