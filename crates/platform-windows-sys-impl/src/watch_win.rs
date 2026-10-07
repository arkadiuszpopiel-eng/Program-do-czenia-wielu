//! Wątek obserwacji jednego katalogu: `ReadDirectoryChangesW` z `OVERLAPPED` i zdarzeniem,
//! `WaitForMultipleObjects` na [we/wy, stop]; zatrzymanie = `SetEvent(stop)` → `CancelIoEx` +
//! `GetOverlappedResult(bWait)` (bufor i `OVERLAPPED` żyją do końca operacji). Pierwsze żądanie jest
//! składane **przed** skanem stanu początkowego — zmiany w tym czasie są buforowane przez system.
//! Wynik 0 bajtów albo `ERROR_NOTIFY_ENUM_DIR` = przepełnienie → pełne przeskanowanie.

#![allow(unsafe_code)]

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use platform_contract::{PlatformError, RescanReason, WatchId, WatchSpec};
use windows::Win32::Foundation::{ERROR_NOTIFY_ENUM_DIR, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
    FILE_NOTIFY_CHANGE, FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME,
    FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING, ReadDirectoryChangesW,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::{INFINITE, ResetEvent, SetEvent, WaitForMultipleObjects};
use windows::core::{HRESULT, PCWSTR};

use crate::notify::{Translator, parse_notify_buffer};
use crate::scan::{scan_dir, stat_path};
use crate::watch::Shared;
use crate::win::{OwnedHandle, manual_event, wide, win_error};

const FILTER: FILE_NOTIFY_CHANGE = FILE_NOTIFY_CHANGE(
    FILE_NOTIFY_CHANGE_FILE_NAME.0
        | FILE_NOTIFY_CHANGE_DIR_NAME.0
        | FILE_NOTIFY_CHANGE_LAST_WRITE.0
        | FILE_NOTIFY_CHANGE_SIZE.0,
);

fn open_dir(dir: &Path) -> Result<OwnedHandle, PlatformError> {
    let name = wide(dir.as_os_str());
    // SAFETY: napis zakończony zerem żyje przez wywołanie; tylko listowanie katalogu.
    let h = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
            None,
        )
    }
    .map_err(|e| win_error("CreateFileW (katalog obserwacji)", &e))?;
    OwnedHandle::new(h).ok_or_else(|| PlatformError::Io("CreateFileW: pusty uchwyt".into()))
}

/// Bufor i `OVERLAPPED` jednego żądania (żyją do zakończenia albo anulowania operacji; tworzone
/// na wątku obserwacji — `OVERLAPPED` nie jest `Send`).
struct Request {
    dir: OwnedHandle,
    io: OwnedHandle,
    buf: Vec<u64>,
    ov: Box<OVERLAPPED>,
    recursive: bool,
}

impl Request {
    fn new(dir: OwnedHandle, io: OwnedHandle, buffer_bytes: usize, recursive: bool) -> Self {
        Self {
            dir,
            io,
            buf: vec![0u64; buffer_bytes.div_ceil(8)],
            ov: Box::default(),
            recursive,
        }
    }

    fn issue(&mut self) -> Result<(), PlatformError> {
        // SAFETY: zdarzenie należy do żądania.
        unsafe { ResetEvent(self.io.raw()) }.map_err(|e| win_error("ResetEvent", &e))?;
        *self.ov = OVERLAPPED {
            hEvent: self.io.raw(),
            ..OVERLAPPED::default()
        };
        let len = u32::try_from(self.buf.len() * 8).unwrap_or(u32::MAX);
        // SAFETY: bufor (wyrównany do 8, długość `len`) i `OVERLAPPED` żyją w `self` do
        // zakończenia operacji — `finish`/`cancel` czekają na nią przed zwolnieniem.
        unsafe {
            ReadDirectoryChangesW(
                self.dir.raw(),
                self.buf.as_mut_ptr().cast(),
                len,
                self.recursive,
                FILTER,
                None,
                Some(&raw mut *self.ov),
                None,
            )
        }
        .map_err(|e| win_error("ReadDirectoryChangesW", &e))
    }

    /// Wynik zakończonej operacji: `Ok(None)` = przepełnienie, `Ok(Some(bajty))`.
    fn finish(&self) -> Result<Option<&[u8]>, windows::core::Error> {
        let mut n = 0u32;
        // SAFETY: zdarzenie operacji zasygnalizowane — operacja zakończona, `OVERLAPPED` ważny.
        unsafe { GetOverlappedResult(self.dir.raw(), &raw const *self.ov, &raw mut n, false) }?;
        let n = usize::try_from(n).unwrap_or(0).min(self.buf.len() * 8);
        if n == 0 {
            return Ok(None);
        }
        // SAFETY: `u8` ma wyrównanie 1; `n` ≤ długość bufora w bajtach; bufor nie jest zapisywany,
        // dopóki trwa pożyczka (następne żądanie dopiero po przetworzeniu).
        Ok(Some(unsafe {
            std::slice::from_raw_parts(self.buf.as_ptr().cast::<u8>(), n)
        }))
    }

    fn cancel(&self) {
        let mut n = 0u32;
        // SAFETY: anulowanie własnej operacji i oczekiwanie na jej koniec przed zwolnieniem bufora.
        unsafe {
            let _ = CancelIoEx(self.dir.raw(), Some(&raw const *self.ov));
            let _ = GetOverlappedResult(self.dir.raw(), &raw const *self.ov, &raw mut n, true);
        }
    }
}

fn rescan(shared: &Shared, id: WatchId, spec: &WatchSpec, reason: RescanReason) {
    let (max, policy) = {
        let set = shared.set();
        (set.policy().max_entries, set.policy().clone())
    };
    let (listing, _) = scan_dir(&spec.dir, spec.recursive, &policy, max);
    shared.apply(id, Vec::new(), Some((reason, listing)));
}

fn process(shared: &Shared, id: WatchId, spec: &WatchSpec, tr: &mut Translator, bytes: &[u8]) {
    let entries = parse_notify_buffer(bytes);
    let out = tr.translate(&spec.dir, entries, stat_path, |p| shared.knows_under(id, p));
    shared.apply(id, out.changes, None);
    if out.rescan {
        rescan(shared, id, spec, RescanReason::DirectoryMoved);
    }
}

fn run(
    shared: &Shared,
    mut req: Request,
    stop: &OwnedHandle,
    spec: &WatchSpec,
    ready: &mpsc::SyncSender<Result<(), PlatformError>>,
    id_rx: &mpsc::Receiver<Option<WatchId>>,
) {
    if let Err(e) = req.issue() {
        let _ = ready.send(Err(e));
        return;
    }
    let _ = ready.send(Ok(()));
    let Ok(Some(id)) = id_rx.recv() else {
        req.cancel();
        return;
    };
    let mut tr = Translator::default();
    let overflow = HRESULT::from_win32(ERROR_NOTIFY_ENUM_DIR.0);
    loop {
        // SAFETY: oba uchwyty zdarzeń ważne przez całe oczekiwanie.
        let woke = unsafe { WaitForMultipleObjects(&[req.io.raw(), stop.raw()], false, INFINITE) };
        if woke != WAIT_OBJECT_0 {
            req.cancel();
            return;
        }
        match req.finish() {
            Ok(Some(bytes)) => process(shared, id, spec, &mut tr, bytes),
            Ok(None) => rescan(shared, id, spec, RescanReason::Overflow),
            Err(e) if e.code() == overflow => rescan(shared, id, spec, RescanReason::Overflow),
            Err(e) => {
                shared.stopped(id, &format!("obserwacja przerwana: {e}"));
                return;
            }
        }
        if let Err(e) = req.issue() {
            shared.stopped(id, &e.to_string());
            return;
        }
    }
}

/// Działająca obserwacja (zatrzymywana w `Drop`).
pub(crate) struct Watcher {
    stop: Arc<OwnedHandle>,
    join: Option<JoinHandle<()>>,
}

impl Watcher {
    /// Uchwyt katalogu → pierwsze żądanie → skan stanu początkowego → rejestracja w zbiorze.
    pub(crate) fn start(
        shared: &Arc<Shared>,
        spec: WatchSpec,
        canonical: &Path,
        buffer_bytes: usize,
    ) -> Result<(WatchId, Self), PlatformError> {
        let (dir, io) = (open_dir(&spec.dir)?, manual_event()?);
        let stop = Arc::new(manual_event()?);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (id_tx, id_rx) = mpsc::sync_channel(1);
        let (thread_shared, thread_stop, thread_spec) =
            (Arc::clone(shared), Arc::clone(&stop), spec.clone());
        let join = std::thread::Builder::new()
            .name("alfa-obserwacja".into())
            .spawn(move || {
                let req = Request::new(dir, io, buffer_bytes, thread_spec.recursive);
                run(
                    &thread_shared,
                    req,
                    &thread_stop,
                    &thread_spec,
                    &ready_tx,
                    &id_rx,
                );
            })
            .map_err(|e| PlatformError::Io(format!("wątek obserwacji: {e}")))?;
        // `Drop` obserwacji zatrzymuje i dołącza wątek; na ścieżkach błędu wątek dostaje `None`
        // (anulowanie żądania) zanim `Drop` na niego zaczeka.
        let watcher = Self {
            stop,
            join: Some(join),
        };
        let failed = match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => None,
            Ok(Err(e)) => Some(e),
            Err(_) => Some(PlatformError::Io(
                "obserwacja: brak odpowiedzi wątku".into(),
            )),
        };
        if let Some(e) = failed {
            let _ = id_tx.send(None);
            return Err(e);
        }
        let (max, policy) = {
            let set = shared.set();
            (set.policy().max_entries, set.policy().clone())
        };
        let listing = scan_dir(&spec.dir, spec.recursive, &policy, max).0;
        let added = shared.set().add(spec, Some(canonical), listing);
        match added {
            Ok(id) => {
                let _ = id_tx.send(Some(id));
                Ok((id, watcher))
            }
            Err(e) => {
                let _ = id_tx.send(None);
                Err(e)
            }
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        // SAFETY: zdarzenie stop należy do obserwacji.
        let _ = unsafe { SetEvent(self.stop.raw()) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
