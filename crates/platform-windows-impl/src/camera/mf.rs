//! Media Foundation — część wspólna: inicjalizacja COM + MF na wątku, atrybuty źródła wideo
//! i wyliczanie kamer (`MFEnumDeviceSources`).

#![allow(unsafe_code)]

use std::ptr::null_mut;

use platform_contract::{CameraDevice, PlatformError};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, IMFAttributes, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_VERSION, MFCreateAttributes,
    MFEnumDeviceSources, MFSTARTUP_LITE, MFShutdown, MFStartup,
};
use windows::Win32::System::Com::{
    COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::core::{Error as WinError, GUID, PWSTR};

use crate::win::{pcwstr, wide, win_error};

/// COM (MTA) + Media Foundation na bieżącym wątku; zwalniane w odwrotnej kolejności.
pub(super) struct MediaFoundation;

impl MediaFoundation {
    pub(super) fn start() -> Result<Self, PlatformError> {
        // SAFETY: wątek należy do nas; para `CoUninitialize` tylko po udanej inicjalizacji.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_err() {
            return Err(win_error("CoInitializeEx", &WinError::from_hresult(hr)));
        }
        // SAFETY: COM jest zainicjalizowany; `MFShutdown` wywołuje `Drop` (para do udanego startu).
        if let Err(e) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE) } {
            // SAFETY: para do udanego `CoInitializeEx` powyżej.
            unsafe { CoUninitialize() };
            return Err(win_error("MFStartup", &e));
        }
        Ok(Self)
    }
}

impl Drop for MediaFoundation {
    fn drop(&mut self) {
        // SAFETY: obiekty MF/COM tego wątku zostały zwolnione wcześniej (pola i zmienne lokalne
        // wychodzą z zasięgu przed strażnikiem); pary do `MFStartup` i `CoInitializeEx`.
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }
}

pub(super) fn attributes(capacity: u32) -> Result<IMFAttributes, PlatformError> {
    let mut out = None;
    // SAFETY: wskaźnik wyjściowy wskazuje na lokalną zmienną `Option<IMFAttributes>`.
    unsafe { MFCreateAttributes(&raw mut out, capacity) }
        .map_err(|e| win_error("MFCreateAttributes", &e))?;
    out.ok_or_else(|| PlatformError::Io("MFCreateAttributes: brak obiektu".into()))
}

pub(super) fn video_capture_attributes(link: Option<&str>) -> Result<IMFAttributes, PlatformError> {
    let attrs = attributes(2)?;
    // SAFETY: stałe GUID żyją statycznie; obiekt atrybutów jest ważny.
    unsafe {
        attrs.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )
    }
    .map_err(|e| win_error("IMFAttributes::SetGUID", &e))?;
    if let Some(link) = link {
        let buffer = wide(link);
        // SAFETY: bufor UTF-16 zakończony zerem żyje do końca wywołania (MF kopiuje napis).
        unsafe {
            attrs.SetString(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
                pcwstr(&buffer),
            )
        }
        .map_err(|e| win_error("IMFAttributes::SetString", &e))?;
    }
    Ok(attrs)
}

fn allocated_string(activate: &IMFActivate, key: &GUID) -> Option<String> {
    let (mut text, mut len) = (PWSTR::null(), 0u32);
    // SAFETY: wskaźniki wyjściowe wskazują na zmienne lokalne; napis zwalniamy `CoTaskMemFree`.
    unsafe {
        activate
            .GetAllocatedString(key, &raw mut text, &raw mut len)
            .ok()?;
        let value = text.to_string().ok();
        CoTaskMemFree(Some(text.0.cast_const().cast()));
        value
    }
}

fn enumerate() -> Result<Vec<CameraDevice>, PlatformError> {
    let attrs = video_capture_attributes(None)?;
    let (mut list, mut count) = (null_mut::<Option<IMFActivate>>(), 0u32);
    // SAFETY: wskaźniki wyjściowe wskazują na zmienne lokalne; tablicę zwalniamy niżej.
    unsafe { MFEnumDeviceSources(&attrs, &raw mut list, &raw mut count) }
        .map_err(|e| win_error("MFEnumDeviceSources", &e))?;
    let mut out = Vec::new();
    if list.is_null() {
        return Ok(out);
    }
    // SAFETY: MF zwróciło tablicę `count` wskaźników interfejsów; przejmujemy każdy element
    // (`take` zwalnia referencję przy wyjściu z pętli), a potem zwalniamy samą tablicę.
    unsafe {
        for slot in std::slice::from_raw_parts_mut(list, count as usize) {
            let Some(activate) = slot.take() else {
                continue;
            };
            let link = allocated_string(
                &activate,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
            );
            let name = allocated_string(&activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME);
            if let Some(id) = link.filter(|l| !l.is_empty()) {
                let name = name.filter(|n| !n.trim().is_empty());
                out.push(CameraDevice {
                    name: name.unwrap_or_else(|| "Kamera".to_owned()),
                    id,
                });
            }
        }
        CoTaskMemFree(Some(list.cast_const().cast()));
    }
    Ok(out)
}

/// Lista kamer — na krótkotrwałym wątku z własnym COM/MF (wywołujący może być wątkiem tokio).
pub(super) fn devices() -> Result<Vec<CameraDevice>, PlatformError> {
    std::thread::Builder::new()
        .name("alfa-camera-list".into())
        .spawn(|| {
            let _mf = MediaFoundation::start()?;
            enumerate()
        })
        .map_err(|e| PlatformError::Io(format!("nie można uruchomić wątku kamer: {e}")))?
        .join()
        .map_err(|_| PlatformError::Io("wątek kamer zakończył się paniką".into()))?
}
