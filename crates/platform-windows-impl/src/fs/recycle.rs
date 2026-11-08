//! Usuwanie do Kosza przez `IFileOperation` (STA) z odbiorem położenia w Koszu
//! (`IFileOperationProgressSink::PostDeleteItem` → `psiNewlyCreated`), żeby cofnięcie było możliwe.
//!
//! Flagi: `FOFX_RECYCLEONDELETE | FOF_ALLOWUNDO` + bez interfejsu (`FOF_SILENT`, `FOF_NOERRORUI`,
//! `FOF_NOCONFIRMATION`), ale z `FOF_WANTNUKEWARNING`: gdy elementu nie da się przenieść do Kosza
//! (za duży, wolumin bez Kosza), powłoka pyta użytkownika zamiast po cichu usuwać trwale.

#![allow(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use platform_contract::PlatformError;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::{
    FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOCONFIRMMKDIR, FOF_NOERRORUI, FOF_SILENT,
    FOF_WANTNUKEWARNING, FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE, FileOperation, IFileOperation,
    IFileOperationProgressSink, IFileOperationProgressSink_Impl, IShellItem,
    SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::{HRESULT, PCWSTR, Ref, Result as WinResult, implement};

use crate::win::{Apartment, pcwstr, run_in_apartment, wide, win_error};

/// Wynik usuwania do Kosza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecycleOutcome {
    /// Element jest w Koszu pod tą ścieżką (`$R…` w `$Recycle.Bin`).
    Recycled(PathBuf),
    /// Kosz niedostępny, użytkownik zgodził się na trwałe usunięcie (nieodwracalne).
    Destroyed,
}

#[derive(Debug, Default)]
struct SinkState {
    delete_hr: Option<HRESULT>,
    recycled: Option<PathBuf>,
}

#[implement(IFileOperationProgressSink)]
struct Sink {
    /// Ścieżka usuwanego elementu małymi literami (przy rekursji powłoka może zgłaszać też dzieci).
    target: String,
    state: Arc<Mutex<SinkState>>,
}

fn display_path(item: &IShellItem) -> Option<PathBuf> {
    // SAFETY: `item` to ważny obiekt powłoki przekazany przez `IFileOperation` na tym wątku STA.
    let raw = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }.ok()?;
    // SAFETY: `raw` to napis zakończony zerem od powłoki; kopiujemy przed zwolnieniem.
    let text = unsafe { raw.to_string() }.ok();
    // SAFETY: bufor z `GetDisplayName` zwalnia się `CoTaskMemFree` dokładnie raz.
    unsafe { CoTaskMemFree(Some(raw.0.cast_const().cast())) };
    text.map(PathBuf::from)
}

impl IFileOperationProgressSink_Impl for Sink_Impl {
    fn StartOperations(&self) -> WinResult<()> {
        Ok(())
    }
    fn FinishOperations(&self, _hr: HRESULT) -> WinResult<()> {
        Ok(())
    }
    fn PreRenameItem(&self, _f: u32, _i: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> {
        Ok(())
    }
    fn PostRenameItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _n: &PCWSTR,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PostMoveItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PreCopyItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PostCopyItem(
        &self,
        _f: u32,
        _i: Ref<IShellItem>,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PreDeleteItem(&self, _f: u32, _i: Ref<IShellItem>) -> WinResult<()> {
        Ok(())
    }
    fn PostDeleteItem(
        &self,
        _f: u32,
        item: Ref<IShellItem>,
        hr: HRESULT,
        created: Ref<IShellItem>,
    ) -> WinResult<()> {
        let path = item.as_ref().and_then(display_path);
        let is_target = path.is_none_or(|p| p.to_string_lossy().to_lowercase() == self.target);
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if is_target || state.delete_hr.is_none() {
            state.delete_hr = Some(hr);
            state.recycled = created.as_ref().and_then(display_path);
        }
        Ok(())
    }
    fn PreNewItem(&self, _f: u32, _d: Ref<IShellItem>, _n: &PCWSTR) -> WinResult<()> {
        Ok(())
    }
    fn PostNewItem(
        &self,
        _f: u32,
        _d: Ref<IShellItem>,
        _n: &PCWSTR,
        _t: &PCWSTR,
        _a: u32,
        _hr: HRESULT,
        _c: Ref<IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _total: u32, _done: u32) -> WinResult<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> WinResult<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> WinResult<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> WinResult<()> {
        Ok(())
    }
}

/// Przenosi element do Kosza (na dedykowanym wątku STA).
pub(crate) fn recycle(path: &Path) -> Result<RecycleOutcome, PlatformError> {
    let simple = simplify_verbatim(path);
    let lower = simple.to_string_lossy().to_lowercase();
    let target = wide(simple);
    run_in_apartment(Apartment::Sta, move || recycle_on_sta(&target, lower))
}

fn recycle_on_sta(target: &[u16], lower: String) -> Result<RecycleOutcome, PlatformError> {
    let err = |ctx: &'static str| move |e: windows::core::Error| win_error(ctx, &e);
    // SAFETY: COM zainicjalizowany jako STA przez `run_in_apartment`; CLSID stały.
    let op: IFileOperation = unsafe { CoCreateInstance(&FileOperation, None, CLSCTX_ALL) }
        .map_err(err("CoCreateInstance(FileOperation)"))?;
    let flags = FOF_ALLOWUNDO
        | FOFX_RECYCLEONDELETE
        | FOF_NOCONFIRMATION
        | FOF_NOCONFIRMMKDIR
        | FOF_NOERRORUI
        | FOF_SILENT
        | FOF_WANTNUKEWARNING
        | FOFX_EARLYFAILURE;
    // SAFETY: `op` to ważny obiekt na bieżącym wątku.
    unsafe { op.SetOperationFlags(flags) }.map_err(err("IFileOperation::SetOperationFlags"))?;
    // SAFETY: `target` jest zakończony zerem i żyje do końca funkcji.
    let item: IShellItem = unsafe { SHCreateItemFromParsingName(pcwstr(target), None) }
        .map_err(err("SHCreateItemFromParsingName"))?;
    let state = Arc::new(Mutex::new(SinkState::default()));
    let sink: IFileOperationProgressSink = Sink {
        target: lower,
        state: Arc::clone(&state),
    }
    .into();
    // SAFETY: `item` i `sink` są ważnymi obiektami COM; `op` trzyma do nich referencje do końca operacji.
    unsafe { op.DeleteItem(&item, &sink) }.map_err(err("IFileOperation::DeleteItem"))?;
    // SAFETY: jw.; operacja wykonuje się synchronicznie na tym wątku.
    unsafe { op.PerformOperations() }.map_err(err("IFileOperation::PerformOperations"))?;
    // SAFETY: jw.
    let aborted = unsafe { op.GetAnyOperationsAborted() }
        .map_err(err("IFileOperation::GetAnyOperationsAborted"))?;
    let state = state.lock().unwrap_or_else(|p| p.into_inner());
    if aborted.as_bool() {
        return Err(PlatformError::Io(
            "usuwanie do Kosza przerwane (anulowano lub odmowa)".into(),
        ));
    }
    match (&state.delete_hr, &state.recycled) {
        (Some(hr), _) if hr.is_err() => Err(win_error(
            "IFileOperation (usuwanie)",
            &windows::core::Error::from_hresult(*hr),
        )),
        (Some(_), Some(bin)) => Ok(RecycleOutcome::Recycled(bin.clone())),
        (Some(_), None) => Ok(RecycleOutcome::Destroyed),
        (None, _) => Err(PlatformError::Io(
            "powłoka nie potwierdziła usunięcia elementu".into(),
        )),
    }
}

/// `\\?\C:\x` → `C:\x`, `\\?\UNC\s\u` → `\\s\u` (powłoka nie przyjmuje ścieżek verbatim).
pub(crate) fn simplify_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

/// Plik metadanych Kosza (`$I…`) odpowiadający elementowi `$R…`.
pub(crate) fn recycle_metadata_sibling(recycled: &Path) -> Option<PathBuf> {
    let name = recycled.file_name()?.to_str()?;
    let rest = name.strip_prefix("$R")?;
    Some(recycled.with_file_name(format!("$I{rest}")))
}
