//! Media Foundation: wątek czytnika źródła kamery (`IMFSourceReader`, wyjście RGB32). Wszystkie
//! obiekty COM żyją na jednym wątku (MTA); wyliczanie kamer i inicjalizacja — `mf.rs`.

#![allow(unsafe_code)]

use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use platform_contract::{CameraStream, PlatformError};
use windows::Win32::Media::MediaFoundation::{
    IMF2DBuffer, IMFMediaSource, IMFMediaType, IMFSample, IMFSourceReader, MF_E_NO_MORE_TYPES,
    MF_E_VIDEO_RECORDING_DEVICE_INVALIDATED, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE,
    MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR, MFCreateDeviceSource,
    MFCreateMediaType, MFCreateSourceReaderFromMediaSource, MFGetStrideForBitmapInfoHeader,
    MFMediaType_Video, MFVideoFormat_I420, MFVideoFormat_IYUV, MFVideoFormat_NV12,
    MFVideoFormat_RGB24, MFVideoFormat_RGB32, MFVideoFormat_UYVY, MFVideoFormat_YUY2,
    MFVideoFormat_YV12,
};
use windows::core::{Error as WinError, GUID, Interface};

use super::mf::{MediaFoundation, attributes, video_capture_attributes};
use super::{Mode, Shared, WinStream, pick_mode, rows_top_down};
use crate::win::win_error;

/// Pierwszy strumień wideo czytnika (`MF_SOURCE_READER_FIRST_VIDEO_STREAM` jako `DWORD`).
const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
/// Limit czekania na otwarcie kamery i pierwszą klatkę.
const OPEN_TIMEOUT: Duration = Duration::from_secs(15);
/// Górna granica listy trybów (ochrona przed sterownikiem z nieskończoną listą).
const MAX_MODES: u32 = 512;
/// Ile kolejnych odczytów bez klatki uznajemy za awarię strumienia.
const MAX_EMPTY_READS: u32 = 300;

/// Otwiera kamerę na dedykowanym wątku i czeka na pierwszą klatkę (albo błąd otwarcia).
pub(super) fn open(id: &str, max_side: u32) -> Result<Box<dyn CameraStream>, PlatformError> {
    let shared = Arc::new(Shared::default());
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = channel();
    let (done_tx, done_rx) = channel();
    let (link, worker_shared, worker_stop) = (id.to_owned(), shared.clone(), stop.clone());
    std::thread::Builder::new()
        .name("alfa-camera".into())
        .spawn(move || {
            if let Err(e) = capture(&link, max_side, &worker_shared, &worker_stop, &ready_tx) {
                // Przed gotowością błąd trafia do `open`, po niej — do `read`.
                worker_shared.fail(e.clone());
                let _ = ready_tx.send(Err(e));
            }
            let _ = done_tx.send(());
        })
        .map_err(|e| PlatformError::Io(format!("nie można uruchomić wątku kamery: {e}")))?;
    match ready_rx.recv_timeout(OPEN_TIMEOUT) {
        Ok(Ok(native)) => Ok(Box::new(WinStream::new(
            shared, stop, done_rx, native, max_side,
        ))),
        Ok(Err(e)) => Err(e),
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            Err(PlatformError::Io(
                "kamera nie odpowiedziała w wyznaczonym czasie".into(),
            ))
        }
    }
}

/// Zamyka źródło przy wyjściu z wątku (bez tego kamera zostaje zajęta do końca procesu).
struct SourceGuard(IMFMediaSource);

impl Drop for SourceGuard {
    fn drop(&mut self) {
        // SAFETY: źródło jest ważne; `Shutdown` wolno wywołać raz, na końcu życia źródła.
        let _ = unsafe { self.0.Shutdown() };
    }
}

fn is_uncompressed(subtype: &GUID) -> bool {
    [
        MFVideoFormat_NV12,
        MFVideoFormat_YUY2,
        MFVideoFormat_UYVY,
        MFVideoFormat_I420,
        MFVideoFormat_IYUV,
        MFVideoFormat_YV12,
        MFVideoFormat_RGB24,
        MFVideoFormat_RGB32,
    ]
    .contains(subtype)
}

/// Rozbija atrybut 64-bitowy na dwie połowy (rozmiar: szerokość/wysokość; klatki: licznik/mianownik).
fn halves(media: &IMFMediaType, key: &GUID) -> Option<(u32, u32)> {
    // SAFETY: obiekt typu jest ważny; klucz to stała GUID.
    let packed = unsafe { media.GetUINT64(key) }.ok()?;
    Some((
        u32::try_from(packed >> 32).ok()?,
        u32::try_from(packed & 0xFFFF_FFFF).ok()?,
    ))
}

fn native_modes(reader: &IMFSourceReader) -> Vec<(IMFMediaType, Mode)> {
    let mut out = Vec::new();
    for index in 0..MAX_MODES {
        // SAFETY: czytnik jest ważny; indeks spoza listy zwraca `MF_E_NO_MORE_TYPES`.
        let media = match unsafe { reader.GetNativeMediaType(VIDEO, index) } {
            Ok(media) => media,
            Err(e) if e.code() == MF_E_NO_MORE_TYPES => break,
            Err(_) => break,
        };
        let Some((width, height)) = halves(&media, &MF_MT_FRAME_SIZE) else {
            continue;
        };
        let fps = halves(&media, &MF_MT_FRAME_RATE).map_or(0, |(n, d)| n / d.max(1));
        // SAFETY: obiekt typu jest ważny; klucz to stała GUID.
        let subtype = unsafe { media.GetGUID(&MF_MT_SUBTYPE) }.unwrap_or_default();
        let mode = Mode {
            width,
            height,
            fps,
            compressed: !is_uncompressed(&subtype),
        };
        out.push((media, mode));
    }
    out
}

fn request_rgb32(reader: &IMFSourceReader) -> Result<(), PlatformError> {
    // SAFETY: nowy typ jest ważny; stałe GUID żyją statycznie; czytnik dobiera dekoder/konwerter.
    unsafe {
        let wanted = MFCreateMediaType().map_err(|e| win_error("MFCreateMediaType", &e))?;
        wanted
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .and_then(|()| wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32))
            .map_err(|e| win_error("IMFMediaType::SetGUID", &e))?;
        reader
            .SetCurrentMediaType(VIDEO, None, &wanted)
            .map_err(|e| win_error("kamera nie potrafi dostarczyć obrazu RGB32", &e))
    }
}

/// Rozmiar i krok wiersza bieżącego typu wyjściowego (krok < 0 = wiersze od dołu).
fn output_layout(reader: &IMFSourceReader) -> Result<(u32, u32, i32), PlatformError> {
    // SAFETY: czytnik jest ważny; typ wyjściowy został ustawiony.
    let current = unsafe { reader.GetCurrentMediaType(VIDEO) }
        .map_err(|e| win_error("IMFSourceReader::GetCurrentMediaType", &e))?;
    let (width, height) = halves(&current, &MF_MT_FRAME_SIZE)
        .filter(|(w, h)| *w > 0 && *h > 0)
        .ok_or_else(|| PlatformError::Io("kamera nie podała rozmiaru klatki".into()))?;
    // Rozmiar pochodzi od sterownika: zanim cokolwiek zaalokujemy, musi mieścić się w limicie obrazu.
    if u64::from(width) * u64::from(height) > platform_contract::MAX_IMAGE_PIXELS {
        return Err(PlatformError::Io(
            "kamera podała zbyt duży rozmiar klatki".into(),
        ));
    }
    // SAFETY: obiekt typu jest ważny. Atrybut jest liczbą ze znakiem zapisaną jako UINT32.
    let stride = unsafe { current.GetUINT32(&MF_MT_DEFAULT_STRIDE) }
        .map(|raw| i32::from_ne_bytes(raw.to_ne_bytes()))
        // SAFETY: czysta funkcja arytmetyczna MF (format FOURCC/D3DFORMAT + szerokość).
        .or_else(|_| unsafe { MFGetStrideForBitmapInfoHeader(MFVideoFormat_RGB32.data1, width) })
        .unwrap_or_else(|_| i32::try_from(u64::from(width) * 4).unwrap_or(i32::MAX));
    Ok((width, height, stride))
}

/// Kopiuje piksele próbki do bufora BGRA z wierszami od góry.
fn sample_pixels(sample: &IMFSample, layout: (u32, u32, i32)) -> Result<Vec<u8>, PlatformError> {
    let (width, height, stride) = layout;
    let bad = || PlatformError::Io("kamera podała klatkę o złym rozmiarze".into());
    // SAFETY: próbka jest ważna; bufory są blokowane i odblokowywane parami; kopiujemy dokładnie
    // tyle bajtów, ile gwarantuje blokada (`Lock2D`: `height` wierszy co `pitch`; `Lock`: `len`).
    unsafe {
        let buffer = sample
            .ConvertToContiguousBuffer()
            .map_err(|e| win_error("IMFSample::ConvertToContiguousBuffer", &e))?;
        if let Ok(planar) = buffer.cast::<IMF2DBuffer>() {
            let (mut first, mut pitch) = (null_mut::<u8>(), 0i32);
            if planar.Lock2D(&raw mut first, &raw mut pitch).is_ok() {
                let row = width as usize * 4;
                let fits = !first.is_null() && pitch.unsigned_abs() as usize >= row;
                let mut out = Vec::with_capacity(if fits { row * height as usize } else { 0 });
                if fits {
                    for y in 0..height as isize {
                        // `first` to wiersz górny; ujemny `pitch` prowadzi w dół pamięci.
                        let line = first.offset(y * pitch as isize);
                        out.extend_from_slice(std::slice::from_raw_parts(line, row));
                    }
                }
                let _ = planar.Unlock2D();
                return if fits { Ok(out) } else { Err(bad()) };
            }
        }
        let (mut data, mut len) = (null_mut::<u8>(), 0u32);
        buffer
            .Lock(&raw mut data, None, Some(&raw mut len))
            .map_err(|e| win_error("IMFMediaBuffer::Lock", &e))?;
        let out = if data.is_null() {
            None
        } else {
            let bytes = std::slice::from_raw_parts(data, len as usize);
            rows_top_down(bytes, stride, width, height)
        };
        let _ = buffer.Unlock();
        out.ok_or_else(bad)
    }
}

fn read_error(e: &WinError) -> PlatformError {
    if e.code() == MF_E_VIDEO_RECORDING_DEVICE_INVALIDATED {
        PlatformError::UnknownResource("kamera została odłączona".into())
    } else {
        win_error("odczyt klatki z kamery", e)
    }
}

/// Jedna próbka: `Ok(None)` = znacznik bez obrazu (czytaj dalej).
fn next_sample(reader: &IMFSourceReader) -> Result<(Option<IMFSample>, u32), PlatformError> {
    let (mut flags, mut sample) = (0u32, None);
    // SAFETY: czytnik w trybie synchronicznym; wskaźniki wyjściowe wskazują na zmienne lokalne.
    unsafe {
        reader.ReadSample(
            VIDEO,
            0,
            None,
            Some(&raw mut flags),
            None,
            Some(&raw mut sample),
        )
    }
    .map_err(|e| read_error(&e))?;
    let has = |flag: i32| flags & flag.cast_unsigned() != 0;
    if has(MF_SOURCE_READERF_ERROR.0) {
        return Err(PlatformError::Io("kamera zgłosiła błąd strumienia".into()));
    }
    if has(MF_SOURCE_READERF_ENDOFSTREAM.0) {
        return Err(PlatformError::UnknownResource(
            "kamera zakończyła strumień (odłączona?)".into(),
        ));
    }
    Ok((sample, flags))
}

/// Ciało wątku kamery: źródło → czytnik → tryb → pętla klatek do `stop` albo błędu.
fn capture(
    link: &str,
    max_side: u32,
    shared: &Shared,
    stop: &AtomicBool,
    ready: &Sender<Result<(u32, u32), PlatformError>>,
) -> Result<(), PlatformError> {
    let _mf = MediaFoundation::start()?;
    let attrs = video_capture_attributes(Some(link))?;
    // SAFETY: atrybuty są ważne; MF tworzy źródło dla wskazanego dowiązania.
    let source = unsafe { MFCreateDeviceSource(&attrs) }
        .map_err(|e| win_error("nie można otworzyć kamery", &e))?;
    let _source = SourceGuard(source.clone());
    let options = attributes(1)?;
    // SAFETY: atrybuty są ważne; flaga włącza konwersję koloru (YUV → RGB32) w czytniku.
    unsafe { options.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1) }
        .map_err(|e| win_error("IMFAttributes::SetUINT32", &e))?;
    // SAFETY: źródło i atrybuty są ważne.
    let reader = unsafe { MFCreateSourceReaderFromMediaSource(&source, &options) }
        .map_err(|e| win_error("nie można utworzyć czytnika kamery", &e))?;
    let modes = native_modes(&reader);
    let plain: Vec<Mode> = modes.iter().map(|(_, mode)| *mode).collect();
    if let Some(index) = pick_mode(&plain, max_side) {
        // SAFETY: typ pochodzi z listy typów natywnych tego czytnika. Odmowa nie jest błędem —
        // zostaje tryb domyślny, a klatki pomniejsza strumień.
        let _ = unsafe { reader.SetCurrentMediaType(VIDEO, None, &modes[index].0) };
    }
    drop(modes);
    request_rgb32(&reader)?;
    let mut layout = output_layout(&reader)?;
    let (mut announced, mut empty) = (false, 0u32);
    while !stop.load(Ordering::SeqCst) {
        let (sample, flags) = next_sample(&reader)?;
        if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0.cast_unsigned() != 0 {
            layout = output_layout(&reader)?;
        }
        let Some(sample) = sample else {
            empty += 1;
            if empty > MAX_EMPTY_READS {
                return Err(PlatformError::Io("kamera nie dostarcza klatek".into()));
            }
            continue;
        };
        empty = 0;
        shared.publish(layout.0, layout.1, sample_pixels(&sample, layout)?);
        if !announced {
            announced = true;
            if ready.send(Ok((layout.0, layout.1))).is_err() {
                break;
            }
        }
    }
    Ok(())
}
