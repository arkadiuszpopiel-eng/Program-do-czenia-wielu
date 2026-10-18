//! Wczytanie PDF przez `lopdf` z limitami i dostęp do obiektów. Strumienie obiektów (`ObjStm`)
//! rozpakowujemy sami w filtrze wczytywania — ze wspólnym budżetem bajtów i licznikiem obiektów
//! (stan w `thread_local`, bo filtr `lopdf` to zwykły wskaźnik funkcji; ekstrakcja ma własny wątek).

use std::cell::Cell;

use lopdf::{DecompressError, Dictionary, Document, Error, LoadOptions, Object, ObjectId};

use super::Limits;

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

#[derive(Debug, Clone, Copy)]
struct Budget {
    objects: usize,
    max_objects: usize,
    left: usize,
    per_stream: usize,
    exceeded: bool,
}

thread_local! {
    static BUDGET: Cell<Budget> = const {
        Cell::new(Budget { objects: 0, max_objects: 0, left: 0, per_stream: 0, exceeded: false })
    };
}

/// Filtr wczytywania: limit liczby obiektów i budżet rozpakowania strumieni obiektów.
fn guard(id: ObjectId, object: &mut Object) -> Option<(ObjectId, Object)> {
    let mut b = BUDGET.with(Cell::get);
    b.objects += 1;
    let mut keep = b.objects <= b.max_objects;
    if keep
        && let Object::Stream(stream) = object
        && stream.dict.has_type(b"ObjStm")
    {
        let cap = b.left.min(b.per_stream);
        let unpacked = if stream.is_compressed() {
            stream.decompress_with_limit(cap)
        } else {
            Ok(())
        };
        keep = match unpacked {
            Ok(()) if stream.content.len() <= cap => {
                b.left -= stream.content.len();
                true
            }
            Ok(()) | Err(Error::Decompress(DecompressError::MemoryLimitExceeded { .. })) => false,
            // Inny błąd rozpakowania: `lopdf` sam pominie wadliwy strumień.
            Err(_) => true,
        };
    }
    if !keep {
        b.exceeded = true;
    }
    BUDGET.with(|c| c.set(b));
    keep.then(|| (id, object.clone()))
}

/// Wczytuje dokument (z pustym hasłem, jeśli zaszyfrowany) albo podaje powód odmowy.
pub(super) fn load(bytes: &[u8], limits: &Limits) -> Result<Document, LoadError> {
    BUDGET.with(|c| {
        c.set(Budget {
            objects: 0,
            max_objects: limits.max_objects,
            left: limits.load_total,
            per_stream: limits.load_stream,
            exceeded: false,
        });
    });
    let options = LoadOptions {
        password: None,
        filter: Some(guard),
        strict: false,
        max_decompressed_size: Some(limits.load_stream),
    };
    let result = Document::load_mem_with_options(bytes, options);
    let exceeded = BUDGET.with(Cell::get).exceeded;
    match result {
        Ok(doc) if doc.is_encrypted() => Err(LoadError::Encrypted),
        Ok(_) if exceeded => Err(LoadError::TooComplex),
        Ok(doc) => Ok(doc),
        Err(Error::Decompress(DecompressError::MemoryLimitExceeded { .. })) => {
            Err(LoadError::TooComplex)
        }
        Err(_) if exceeded => Err(LoadError::TooComplex),
        Err(Error::InvalidPassword | Error::Decryption(_)) => Err(LoadError::Encrypted),
        Err(_) => Err(LoadError::Broken),
    }
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
