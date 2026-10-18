//! Strumienie obiektów (`ObjStm`) rozpakowujemy i parsujemy sami. `lopdf` parsuje każdy obiekt
//! od jego przesunięcia do końca strumienia, więc powtórzone albo zachodzące na siebie przesunięcia
//! dają tysiące kopii dużego obiektu z kilku KB pliku (brak pamięci → koniec procesu). Tu każdy
//! obiekt dostaje tylko swój wycinek — do następnego przesunięcia — i każdy wycinek liczymy
//! w budżecie rozpakowanych bajtów.

use lopdf::xref::XrefEntry;
use lopdf::{DecompressError, Dictionary, Document, Error, Object, ObjectId, ObjectStream, Stream};

/// Budżet wczytywania: obiekty i rozpakowane (oraz parsowane) bajty strumieni obiektów.
#[derive(Debug, Clone, Copy)]
pub(super) struct Budget {
    pub(super) objects: usize,
    pub(super) max_objects: usize,
    pub(super) left: usize,
    pub(super) per_stream: usize,
    /// Surowe bajty strumieni najwyższego poziomu, które jeszcze mieszczą się w pliku.
    pub(super) raw_left: usize,
    pub(super) exceeded: bool,
}

impl Budget {
    /// Liczy obiekt; `false` — limit obiektów przekroczony.
    pub(super) fn object(&mut self) -> bool {
        self.objects += 1;
        if self.objects > self.max_objects {
            self.exceeded = true;
        }
        !self.exceeded
    }

    /// Pobiera `n` bajtów z budżetu; `false` — za mało.
    fn charge(&mut self, n: usize) -> bool {
        match self.left.checked_sub(n) {
            Some(left) => {
                self.left = left;
                true
            }
            None => {
                self.exceeded = true;
                false
            }
        }
    }
}

/// Obiekty strumienia `stream` albo `None`, gdy strumień jest wadliwy.
fn objects(mut stream: Stream, budget: &mut Budget) -> Option<Vec<(ObjectId, Object)>> {
    let cap = budget.left.min(budget.per_stream);
    if stream.is_compressed() {
        match stream.decompress_with_limit(cap) {
            Ok(()) => {}
            Err(Error::Decompress(DecompressError::MemoryLimitExceeded { .. })) => {
                budget.exceeded = true;
                return None;
            }
            // Inny błąd rozpakowania: wadliwy strumień pomijamy (jak `lopdf`).
            Err(_) => return None,
        }
    }
    if stream.content.len() > cap || !budget.charge(stream.content.len()) {
        budget.exceeded = true;
        return None;
    }
    let content = stream.content;
    let first = usize::try_from(stream.dict.get(b"First").ok()?.as_i64().ok()?).ok()?;
    let index = std::str::from_utf8(content.get(..first)?).ok()?;
    let numbers: Vec<Option<usize>> = index
        .split_whitespace()
        .map(|n| n.parse::<usize>().ok())
        .collect();
    let entries: Vec<(u32, usize)> = numbers
        .chunks_exact(2)
        .filter_map(|pair| {
            let id = u32::try_from(pair[0]?).ok()?;
            let offset = first.checked_add(pair[1]?)?;
            (offset < content.len()).then_some((id, offset))
        })
        .collect();
    let mut starts: Vec<usize> = entries.iter().map(|&(_, o)| o).collect();
    starts.sort_unstable();
    starts.dedup();
    let mut out = Vec::new();
    for (id, offset) in entries {
        let next = starts.partition_point(|&s| s <= offset);
        let end = starts.get(next).copied().unwrap_or(content.len());
        let slice = &content[offset..end];
        if !budget.object() || !budget.charge(slice.len()) {
            return Some(out);
        }
        if let Some(object) = parse_one(id, slice) {
            out.push(((id, 0), object));
        }
    }
    Some(out)
}

/// Jeden obiekt z wycinka — przez publiczny parser `lopdf` na jednoelementowym strumieniu.
fn parse_one(id: u32, slice: &[u8]) -> Option<Object> {
    let header = format!("{id} 0 ").into_bytes();
    let mut dict = Dictionary::new();
    dict.set("N", 1);
    dict.set("First", i64::try_from(header.len()).ok()?);
    let content = [header.as_slice(), slice, b" "].concat();
    let mut parsed = ObjectStream::new_with_limit(&Stream::new(dict, content), None).ok()?;
    parsed.objects.remove(&(id, 0))
}

/// Dokłada obiekty strumieni `pending` do dokumentu: tylko te, które xref przypisuje temu
/// kontenerowi (albo nie przypisuje żadnemu), i nigdy nie zastępuje obiektów najwyższego poziomu.
pub(super) fn unpack(doc: &mut Document, pending: Vec<(ObjectId, Object)>, budget: &mut Budget) {
    for ((container, _), object) in pending {
        if budget.exceeded {
            return;
        }
        let Object::Stream(stream) = object else {
            continue;
        };
        let Some(found) = objects(stream, budget) else {
            continue;
        };
        for (id, object) in found {
            let ours = match doc.reference_table.get(id.0) {
                Some(XrefEntry::Compressed { container: c, .. }) => *c == container,
                _ => true,
            };
            if ours {
                doc.objects.entry(id).or_insert(object);
            }
        }
    }
}
