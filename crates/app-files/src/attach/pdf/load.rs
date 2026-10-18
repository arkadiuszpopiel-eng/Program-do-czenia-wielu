//! Wczytanie PDF przez `lopdf` z limitami i dostęp do obiektów. Bajty najpierw przygotowujemy
//! (`prepare`: ukryte `/Encrypt` i pośrednie `/Length`), żeby `lopdf` szedł tylko ścieżką z naszym
//! filtrem. Filtr liczy obiekty i surowe bajty strumieni, a strumienie obiektów (`ObjStm`) odkłada —
//! parsujemy je sami (`objstm`) po odszyfrowaniu (`streams`). Stan filtra w `thread_local`, bo filtr
//! `lopdf` to zwykły wskaźnik funkcji; ekstrakcja ma własny wątek.

use std::cell::{Cell, RefCell};

use lopdf::{DecompressError, Dictionary, Document, Error, LoadOptions, Object, ObjectId};

use super::Limits;
use super::objstm::{self, Budget};
use super::prepare;
use super::streams::{self, Layout};

/// Dlaczego PDF nie został wczytany.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum LoadError {
    /// Zaszyfrowany hasłem użytkownika (albo nieobsługiwaną metodą).
    Encrypted,
    /// Przekroczony limit obiektów albo rozpakowanych danych.
    TooComplex,
    /// Uszkodzony, niepełny albo to nie PDF.
    Broken,
}

const EMPTY: Budget = Budget {
    objects: 0,
    max_objects: 0,
    left: 0,
    per_stream: 0,
    raw_left: 0,
    exceeded: false,
};

thread_local! {
    static BUDGET: Cell<Budget> = const { Cell::new(EMPTY) };
    static PENDING: RefCell<Vec<(ObjectId, Object)>> = const { RefCell::new(Vec::new()) };
}

/// Filtr wczytywania: limit obiektów i surowych bajtów strumieni; strumienie obiektów odkłada.
fn guard(id: ObjectId, object: &mut Object) -> Option<(ObjectId, Object)> {
    let mut b = BUDGET.with(Cell::get);
    let mut keep = b.object();
    let mut pending = false;
    if keep && let Object::Stream(stream) = object {
        match b.raw_left.checked_sub(stream.content.len()) {
            Some(left) => b.raw_left = left,
            None => {
                b.exceeded = true;
                keep = false;
            }
        }
        pending = keep && stream.dict.has_type(b"ObjStm");
    }
    BUDGET.with(|c| c.set(b));
    if pending {
        let taken = std::mem::replace(object, Object::Null);
        PENDING.with(|p| p.borrow_mut().push((id, taken)));
        return None;
    }
    keep.then(|| (id, object.clone()))
}

/// Wczytuje dokument (z pustym hasłem, jeśli zaszyfrowany) albo podaje powód odmowy.
pub(super) fn load(bytes: &[u8], limits: &Limits) -> Result<Document, LoadError> {
    let prepared = prepare::prepare(bytes);
    // Dane strumieni nakładające się na siebie (zagnieżdżone strumienie) — każdy byłby kopią.
    if prepared.direct_lengths > (bytes.len() as u64).saturating_mul(2) {
        return Err(LoadError::TooComplex);
    }
    BUDGET.with(|c| {
        c.set(Budget {
            max_objects: limits.max_objects,
            left: limits.load_total,
            per_stream: limits.load_stream,
            raw_left: bytes.len(),
            ..EMPTY
        });
    });
    PENDING.with(|p| p.borrow_mut().clear());
    let options = LoadOptions {
        password: None,
        filter: Some(guard),
        strict: false,
        max_decompressed_size: Some(limits.load_stream),
    };
    let result = Document::load_mem_with_options(&prepared.bytes, options);
    let mut pending = PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()));
    let mut budget = BUDGET.with(Cell::get);
    let mut doc = match result {
        Ok(doc) if doc.is_encrypted() => return Err(LoadError::Encrypted),
        Ok(_) if budget.exceeded => return Err(LoadError::TooComplex),
        Ok(doc) => doc,
        Err(Error::Decompress(DecompressError::MemoryLimitExceeded { .. })) => {
            return Err(LoadError::TooComplex);
        }
        Err(_) if budget.exceeded => return Err(LoadError::TooComplex),
        Err(Error::InvalidPassword | Error::Decryption(_)) => return Err(LoadError::Encrypted),
        Err(_) => return Err(LoadError::Broken),
    };
    let start = prepare::pdf_start(&prepared.bytes);
    let layout = Layout::new(&doc, &prepared.bytes[start..]);
    streams::fill_all(&mut doc, &mut pending, &layout, &mut budget);
    let state = streams::unlock(&mut doc, &mut pending).map_err(|()| LoadError::Encrypted)?;
    objstm::unpack(&mut doc, pending, &mut budget);
    let late = streams::fill_all(&mut doc, &mut [], &layout, &mut budget);
    if let Some(state) = &state {
        streams::decrypt_contents(&mut doc, state, &late);
    }
    if budget.exceeded {
        return Err(LoadError::TooComplex);
    }
    Ok(doc)
}

/// Obiekt po rozwinięciu referencji (z identyfikatorem ostatniej referencji).
pub(super) fn resolve<'a>(
    doc: &'a Document,
    obj: &'a Object,
) -> Option<(Option<ObjectId>, &'a Object)> {
    doc.dereference(obj).ok()
}

/// Wartość klucza po rozwinięciu referencji.
pub(super) fn get<'a>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    resolve(doc, dict.get(key).ok()?).map(|(_, o)| o)
}

/// Słownik pod kluczem.
pub(super) fn get_dict<'a>(
    doc: &'a Document,
    dict: &'a Dictionary,
    key: &[u8],
) -> Option<&'a Dictionary> {
    get(doc, dict, key)?.as_dict().ok()
}

/// Nazwa pod kluczem.
pub(super) fn get_name<'a>(
    doc: &'a Document,
    dict: &'a Dictionary,
    key: &[u8],
) -> Option<&'a [u8]> {
    get(doc, dict, key)?.as_name().ok()
}

/// Liczba (całkowita albo rzeczywista, także przez referencję).
pub(super) fn num(doc: &Document, obj: &Object) -> Option<f64> {
    match resolve(doc, obj)?.1 {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(f64::from(*r)).filter(|r| r.is_finite()),
        _ => None,
    }
}

/// Liczba pod kluczem.
pub(super) fn get_num(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<f64> {
    num(doc, dict.get(key).ok()?)
}

/// Element zasobów strony (`/Font`, `/XObject`) o nazwie `name`, z identyfikatorem obiektu.
pub(super) fn resource<'a>(
    doc: &'a Document,
    resources: Option<&'a Dictionary>,
    kind: &[u8],
    name: &[u8],
) -> Option<(Option<ObjectId>, &'a Object)> {
    let group = get_dict(doc, resources?, kind)?;
    resolve(doc, group.get(name).ok()?)
}

/// Zasoby strony — własne albo odziedziczone po przodkach w drzewie stron.
pub(super) fn page_resources<'a>(
    doc: &'a Document,
    page: &'a Dictionary,
) -> Option<&'a Dictionary> {
    let mut node = page;
    for _ in 0..32 {
        if let Some(found) = get_dict(doc, node, b"Resources") {
            return Some(found);
        }
        node = get_dict(doc, node, b"Parent")?;
    }
    None
}
