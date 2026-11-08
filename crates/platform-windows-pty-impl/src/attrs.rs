//! Lista atrybutów procesu (`PROC_THREAD_ATTRIBUTE_LIST`) w buforze wyrównanym do słowa
//! maszynowego (przegląd bezpieczeństwa #2, P2-05): `InitializeProcThreadAttributeList` wymaga
//! wskaźnika wyrównanego jak struktura z polami wskaźnikowymi, a `Vec<u8>` gwarantuje tylko
//! wyrównanie 1. Bufor `Vec<usize>` o długości zaokrąglonej w górę; `Drop` woła
//! `DeleteProcThreadAttributeList` tylko po udanej inicjalizacji (także na ścieżkach błędu).

#![allow(unsafe_code)]

use std::mem::size_of;

use platform_contract::PlatformError;
use windows::Win32::System::Threading::{
    DeleteProcThreadAttributeList, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
};

/// Liczba słów `usize` mieszczących `bytes` bajtów (co najmniej 1).
pub(crate) fn words_for(bytes: usize) -> usize {
    bytes.div_ceil(size_of::<usize>()).max(1)
}

/// Zainicjalizowana lista atrybutów (zwalniana w `Drop`).
pub(crate) struct AttrList {
    buf: Vec<usize>,
}

impl AttrList {
    /// Lista na `count` atrybutów.
    pub(crate) fn new(count: u32) -> Result<Self, PlatformError> {
        let mut size = 0usize;
        // SAFETY: pierwsze wywołanie tylko zwraca wymagany rozmiar listy (błąd jest oczekiwany).
        let _ = unsafe { InitializeProcThreadAttributeList(None, count, None, &raw mut size) };
        let words = words_for(size);
        let mut buf = vec![0usize; words];
        let mut size = words * size_of::<usize>();
        let list = LPPROC_THREAD_ATTRIBUTE_LIST(buf.as_mut_ptr().cast());
        // SAFETY: bufor wyrównany do `usize`, o rozmiarze ≥ wymaganego; po sukcesie zwalnia `Drop`.
        unsafe { InitializeProcThreadAttributeList(Some(list), count, None, &raw mut size) }
            .map_err(|e| {
                PlatformError::Io(format!(
                    "InitializeProcThreadAttributeList: 0x{:08X} {}",
                    e.code().0,
                    e.message()
                ))
            })?;
        Ok(Self { buf })
    }

    /// Wskaźnik listy (ważny, dopóki żyje `self`; sterta `Vec` nie przesuwa się przy przeniesieniu).
    pub(crate) fn as_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        LPPROC_THREAD_ATTRIBUTE_LIST(self.buf.as_mut_ptr().cast())
    }
}

impl Drop for AttrList {
    fn drop(&mut self) {
        // SAFETY: lista zainicjalizowana w `new` (inaczej `AttrList` nie powstaje), zwalniana raz.
        unsafe { DeleteProcThreadAttributeList(self.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_is_word_aligned_and_large_enough() {
        assert_eq!(words_for(0), 1);
        assert_eq!(words_for(1), 1);
        assert_eq!(words_for(size_of::<usize>() + 1), 2);
        let mut list = AttrList::new(2).unwrap();
        let ptr = list.as_ptr().0 as usize;
        assert_eq!(ptr % std::mem::align_of::<usize>(), 0);
    }
}
