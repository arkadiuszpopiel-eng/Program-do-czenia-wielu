//! Format wektora dla `vec0` (sqlite-vec): `float32` little-endian, bez nagłówka.

/// Wektor → BLOB dla kolumny `float[N]` w `vec0`.
pub fn vector_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// BLOB → wektor; `None`, gdy długość nie jest wielokrotnością 4.
pub fn blob_to_vector(blob: &[u8]) -> Option<Vec<f32>> {
    if !blob.len().is_multiple_of(4) {
        return None;
    }
    Some(
        blob.chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let v = vec![0.0, -1.5, 3.25, f32::MIN_POSITIVE];
        assert_eq!(blob_to_vector(&vector_to_blob(&v)).unwrap(), v);
        assert!(blob_to_vector(&[1, 2, 3]).is_none());
    }
}
