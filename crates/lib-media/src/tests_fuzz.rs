//! Testy właściwości: losowe bajty i mutacje poprawnych plików nigdy nie powodują paniki,
//! zapętlenia ani odczytu ponad budżet; wymiary i czas pochodzą wyłącznie z nagłówka.

use proptest::prelude::*;

use crate::samples;
use crate::*;

/// Źródło liczące przeczytane bajty.
struct Counting<'a> {
    inner: SliceSource<'a>,
    read: u64,
}

impl ByteSource for Counting<'_> {
    fn size(&self) -> u64 {
        self.inner.size()
    }

    fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, MediaError> {
        let out = self.inner.read_at(offset, len)?;
        self.read += out.len() as u64;
        Ok(out)
    }
}

fn corpus() -> Vec<Vec<u8>> {
    vec![
        samples::png(64, 32, 3),
        samples::jpeg(800, 600),
        samples::gif(16, 16, 4, 7),
        samples::bmp(20, 20),
        samples::webp_lossless(50, 40),
        samples::wav(44_100, 2, 16, 50),
        samples::mp3(77),
        samples::flac(44_100, 2, 4410),
        samples::ogg_opus(1000),
        samples::mp4(320, 240, 5000, true),
        samples::mp4(320, 240, 5000, false),
    ]
}

fn probe_counted(bytes: &[u8], limits: &Limits) -> (Result<MediaInfo, MediaError>, u64) {
    let mut src = Counting {
        inner: SliceSource::new(bytes),
        read: 0,
    };
    let r = probe(&mut src, limits);
    (r, src.read)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = probe_bytes(&bytes);
    }

    #[test]
    fn random_bytes_after_signature_never_panic(
        sig in 0usize..11,
        tail in proptest::collection::vec(any::<u8>(), 0..2048),
    ) {
        let mut bytes = corpus()[sig][..16.min(corpus()[sig].len())].to_vec();
        bytes.extend_from_slice(&tail);
        let _ = probe_bytes(&bytes);
    }

    #[test]
    fn mutated_samples_stay_within_budget(
        which in 0usize..11,
        flips in proptest::collection::vec((any::<usize>(), any::<u8>()), 1..24),
        cut in any::<usize>(),
    ) {
        let mut bytes = corpus()[which].clone();
        for (at, v) in flips {
            let i = at % bytes.len();
            bytes[i] = v;
        }
        if cut % 3 == 0 {
            bytes.truncate(cut % (bytes.len() + 1));
        }
        let limits = Limits { max_read_bytes: 256 * 1024, ..Limits::default() };
        let (r, read) = probe_counted(&bytes, &limits);
        prop_assert!(read <= 256 * 1024);
        if let Ok(info) = r {
            prop_assert_eq!(info.size_bytes, bytes.len() as u64);
        }
    }
}

#[test]
fn every_sample_is_recognized() {
    for bytes in corpus() {
        let info = probe_bytes(&bytes).unwrap();
        assert!(!info.format.is_empty() && !info.mime.is_empty(), "{info:?}");
    }
}
