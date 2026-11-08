//! Konwersje próbek dla bufora urządzenia (wątek RT: bez alokacji, na buforach przygotowanych
//! wcześniej). WASAPI w trybie współdzielonym z autokonwersją dostaje zawsze `f32` LE.

/// `f32` → bajty LE (`dst.len() >= 4 · src.len()`); zwraca liczbę zapisanych bajtów.
pub fn f32_to_le_bytes(src: &[f32], dst: &mut [u8]) -> usize {
    let n = src.len().min(dst.len() / 4);
    for (s, d) in src[..n].iter().zip(dst.chunks_exact_mut(4)) {
        d.copy_from_slice(&s.to_le_bytes());
    }
    n * 4
}

/// Bajty LE → `f32`; zwraca liczbę próbek.
pub fn le_bytes_to_f32(src: &[u8], dst: &mut [f32]) -> usize {
    let n = (src.len() / 4).min(dst.len());
    for (s, d) in src.chunks_exact(4).zip(dst[..n].iter_mut()) {
        *d = f32::from_le_bytes([s[0], s[1], s[2], s[3]]);
    }
    n
}

/// Miksuje blok przeplatany do mono w miejscu (`dst.len() >= src.len() / channels`).
pub fn downmix_into(src: &[f32], channels: usize, dst: &mut [f32]) -> usize {
    let ch = channels.max(1);
    let n = (src.len() / ch).min(dst.len());
    let scale = 1.0 / ch as f32;
    for (frame, d) in src.chunks_exact(ch).zip(dst[..n].iter_mut()) {
        *d = frame.iter().sum::<f32>() * scale;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_bounds() {
        let src = [0.5f32, -1.0, 0.25];
        let mut bytes = [0u8; 12];
        assert_eq!(f32_to_le_bytes(&src, &mut bytes), 12);
        let mut back = [0f32; 3];
        assert_eq!(le_bytes_to_f32(&bytes, &mut back), 3);
        assert_eq!(back, src);
        let mut short = [0u8; 5];
        assert_eq!(f32_to_le_bytes(&src, &mut short), 4);
        let mut mono = [0f32; 2];
        assert_eq!(downmix_into(&[1.0, 0.0, 0.5, 0.5], 2, &mut mono), 2);
        assert_eq!(mono, [0.5, 0.5]);
    }
}
