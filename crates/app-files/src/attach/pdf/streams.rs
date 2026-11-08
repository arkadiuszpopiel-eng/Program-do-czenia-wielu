//! Po wczytaniu: treść strumieni z ukrytym pośrednim `/Length` (czytana z surowych bajtów, tylko
//! do następnego obiektu i z budżetem wielkości pliku) oraz odszyfrowanie pustym hasłem
//! użytkownika — to samo, co robi `lopdf`, ale bez jego ścieżki dokumentów zaszyfrowanych.

use lopdf::encryption::decrypt_object;
use lopdf::xref::XrefEntry;
use lopdf::{Document, EncryptionState, Object, ObjectId, Stream};

use super::objstm::Budget;
use super::prepare::{HIDDEN_ENCRYPT, HIDDEN_LENGTH};

/// Surowe bajty (od `%PDF-`) i granice obiektów z xref.
pub(super) struct Layout<'b> {
    buf: &'b [u8],
    bounds: Vec<usize>,
}

impl<'b> Layout<'b> {
    pub(super) fn new(doc: &Document, buf: &'b [u8]) -> Self {
        let mut bounds: Vec<usize> = doc
            .reference_table
            .entries
            .values()
            .filter_map(|e| match e {
                XrefEntry::Normal { offset, .. } => usize::try_from(*offset).ok(),
                _ => None,
            })
            .collect();
        if doc.xref_start > 0 {
            bounds.push(doc.xref_start);
        }
        bounds.sort_unstable();
        bounds.dedup();
        Self { buf, bounds }
    }

    /// Dane od `start` o długości `len`: przed następnym obiektem i zakończone `endstream`.
    fn data(&self, start: usize, len: usize) -> Option<&'b [u8]> {
        let end = start.checked_add(len)?;
        let next = self.bounds.partition_point(|&b| b <= start);
        let limit = self
            .bounds
            .get(next)
            .copied()
            .unwrap_or(self.buf.len())
            .min(self.buf.len());
        if end > limit {
            return None;
        }
        let rest = &self.buf[end..];
        let rest = rest
            .strip_prefix(b"\r\n")
            .or_else(|| rest.strip_prefix(b"\n"))
            .or_else(|| rest.strip_prefix(b"\r"))
            .unwrap_or(rest);
        rest.starts_with(b"endstream")
            .then(|| &self.buf[start..end])
    }
}

/// Długość z ukrytego klucza, jeśli strumień czeka na treść.
fn wanted(doc: &Document, object: &Object) -> Option<usize> {
    let stream = object.as_stream().ok()?;
    if !stream.content.is_empty() || stream.start_position.is_none() {
        return None;
    }
    let value = stream.dict.get(HIDDEN_LENGTH).ok()?;
    let length = doc.dereference(value).ok()?.1.as_i64().ok()?;
    usize::try_from(length).ok()
}

/// Wpisuje treść `len` bajtów; `false` — dane nie pasują albo budżet wyczerpany.
fn fill(stream: &mut Stream, len: usize, layout: &Layout<'_>, budget: &mut Budget) -> bool {
    let Some(data) = stream.start_position.and_then(|s| layout.data(s, len)) else {
        return false;
    };
    let Some(left) = budget.raw_left.checked_sub(data.len()) else {
        budget.exceeded = true;
        return false;
    };
    budget.raw_left = left;
    stream.dict.remove(HIDDEN_LENGTH);
    stream.set_content(data.to_vec());
    true
}

/// Uzupełnia strumienie dokumentu i `pending`; zwraca identyfikatory uzupełnionych w dokumencie.
pub(super) fn fill_all(
    doc: &mut Document,
    pending: &mut [(ObjectId, Object)],
    layout: &Layout<'_>,
    budget: &mut Budget,
) -> Vec<ObjectId> {
    for (_, object) in pending.iter_mut() {
        if let Some(len) = wanted(doc, object)
            && let Object::Stream(stream) = object
        {
            fill(stream, len, layout, budget);
        }
    }
    let todo: Vec<(ObjectId, usize)> = doc
        .objects
        .iter()
        .filter_map(|(id, object)| Some((*id, wanted(doc, object)?)))
        .collect();
    let mut filled = Vec::new();
    for (id, len) in todo {
        if let Some(Object::Stream(stream)) = doc.objects.get_mut(&id)
            && fill(stream, len, layout, budget)
        {
            filled.push(id);
        }
    }
    filled
}

/// Odszyfrowanie pustym hasłem, jeśli trailer miał (ukryte) `/Encrypt`: `Ok(None)` — plik
/// niezaszyfrowany, `Err(())` — potrzebne hasło albo nieobsługiwana metoda.
pub(super) fn unlock(
    doc: &mut Document,
    pending: &mut [(ObjectId, Object)],
) -> Result<Option<EncryptionState>, ()> {
    let Some(value) = doc.trailer.remove(HIDDEN_ENCRYPT) else {
        return Ok(None);
    };
    let encrypt = value.as_reference().map_err(|_| ())?;
    doc.trailer.set("Encrypt", value);
    if doc.authenticate_password("").is_err() {
        return Err(());
    }
    let state = EncryptionState::decode(&*doc, "").map_err(|_| ())?;
    for (id, object) in doc.objects.iter_mut() {
        if *id != encrypt {
            let _ = decrypt_object(&state, *id, object);
        }
    }
    for (id, object) in pending.iter_mut() {
        let _ = decrypt_object(&state, *id, object);
    }
    doc.objects.remove(&encrypt);
    doc.trailer.remove(b"Encrypt");
    Ok(Some(state))
}

/// Odszyfrowuje samą treść strumieni uzupełnionych po odszyfrowaniu słowników.
pub(super) fn decrypt_contents(doc: &mut Document, state: &EncryptionState, ids: &[ObjectId]) {
    for id in ids {
        if let Some(Object::Stream(stream)) = doc.objects.get_mut(id) {
            let content = std::mem::take(&mut stream.content);
            let mut copy = Object::Stream(Stream::new(stream.dict.clone(), content));
            let _ = decrypt_object(state, *id, &mut copy);
            if let Object::Stream(plain) = copy {
                stream.set_content(plain.content);
            }
        }
    }
}
