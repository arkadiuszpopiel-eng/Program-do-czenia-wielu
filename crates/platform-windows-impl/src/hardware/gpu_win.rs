//! Karty graficzne niezależnie od producenta: szczegóły z DXCore (LUID, „zintegrowana”, wersja
//! sterownika) i bieżące zużycie z liczników wydajności systemu (PDH, nazwy angielskie — działają
//! w każdej wersji językowej Windows). NVIDIA, AMD i Intel są czytane tą samą drogą.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::mem::size_of;
use std::time::Duration;

use platform_contract::{GpuUsage, PlatformError};
use windows::Win32::Foundation::LUID;
use windows::Win32::Graphics::DXCore::{
    DXCORE_ADAPTER_ATTRIBUTE_D3D12_CORE_COMPUTE, DXCoreAdapterProperty, DXCoreCreateAdapterFactory,
    DriverVersion, IDXCoreAdapter, IDXCoreAdapterFactory, IDXCoreAdapterList, InstanceLuid,
    IsIntegrated,
};
use windows::Win32::System::Performance::{
    PDH_CSTATUS_VALID_DATA, PDH_FMT, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_FMT_LARGE,
    PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA, PdhAddEnglishCounterW, PdhCloseQuery,
    PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
};
use windows::core::{PCWSTR, w};

use super::pdh_names::{Readings, assemble};
use crate::win::from_wide;

/// `PDH_CSTATUS_NEW_DATA` — pierwsza wartość licznika (także poprawna).
const PDH_CSTATUS_NEW_DATA: u32 = 1;
/// Odstęp między dwoma odczytami liczników obciążenia (licznik „Utilization” jest przyrostowy).
const LOAD_SAMPLE: Duration = Duration::from_millis(250);

/// Szczegóły adaptera z DXCore.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Details {
    pub luid: u64,
    pub integrated: Option<bool>,
    pub driver_version: Option<String>,
}

/// LUID jako jedna liczba (jak w nazwach liczników wydajności).
pub(crate) fn luid_u64(luid: LUID) -> u64 {
    #[allow(clippy::cast_sign_loss)]
    let high = luid.HighPart as u32;
    (u64::from(high) << 32) | u64::from(luid.LowPart)
}

/// Wersja sterownika z liczby DXCore (cztery pola po 16 bitów, od najstarszego).
pub(crate) fn driver_text(raw: u64) -> String {
    let part = |shift: u32| (raw >> shift) & 0xFFFF;
    format!("{}.{}.{}.{}", part(48), part(32), part(16), part(0))
}

/// Odczyt właściwości stałego rozmiaru.
///
/// # Safety
/// `T` musi odpowiadać typowi właściwości w dokumentacji DXCore (bajt, `u64`, `LUID`).
unsafe fn property<T: Default>(adapter: &IDXCoreAdapter, what: DXCoreAdapterProperty) -> Option<T> {
    // SAFETY: zapytanie o obsługę właściwości ważnego adaptera.
    if !unsafe { adapter.IsPropertySupported(what) } {
        return None;
    }
    let mut value = T::default();
    // SAFETY: bufor ma dokładnie `size_of::<T>()` bajtów; typ zgodny z właściwością (wymóg funkcji).
    unsafe { adapter.GetProperty(what, size_of::<T>(), (&raw mut value).cast::<c_void>()) }
        .ok()
        .map(|()| value)
}

/// Szczegóły wszystkich adapterów obliczeniowych (DXCore). Brak DXCore (stary system) — pusto.
pub(crate) fn details() -> Vec<Details> {
    // SAFETY: fabryka DXCore bez inicjalizacji COM; brak dxcore.dll → błąd → pusta lista.
    let Ok(factory) = (unsafe { DXCoreCreateAdapterFactory::<IDXCoreAdapterFactory>() }) else {
        return Vec::new();
    };
    // SAFETY: lista adapterów z filtrem atrybutu (stały GUID); obliczenia obejmują też grafikę.
    let Ok(list) = (unsafe {
        factory
            .CreateAdapterList::<IDXCoreAdapterList>(&[DXCORE_ADAPTER_ATTRIBUTE_D3D12_CORE_COMPUTE])
    }) else {
        return Vec::new();
    };
    // SAFETY: ważna lista.
    let count = unsafe { list.GetAdapterCount() };
    let mut out = Vec::new();
    for index in 0..count {
        // SAFETY: indeks z zakresu listy.
        let Ok(adapter) = (unsafe { list.GetAdapter::<IDXCoreAdapter>(index) }) else {
            continue;
        };
        // SAFETY: `InstanceLuid` to `LUID`, `IsIntegrated` — bajt logiczny, `DriverVersion` — `u64`.
        let (luid, integrated, driver) = unsafe {
            (
                property::<LUID>(&adapter, InstanceLuid),
                property::<u8>(&adapter, IsIntegrated),
                property::<u64>(&adapter, DriverVersion),
            )
        };
        let Some(luid) = luid else {
            continue;
        };
        out.push(Details {
            luid: luid_u64(luid),
            integrated: integrated.map(|b| b != 0),
            driver_version: driver.filter(|v| *v != 0).map(driver_text),
        });
    }
    out
}

/// Zapytanie PDH zamykane przy wyjściu.
struct Query(PDH_HQUERY);

impl Drop for Query {
    fn drop(&mut self) {
        // SAFETY: uchwyt z `PdhOpenQueryW`, zamykany raz.
        let _ = unsafe { PdhCloseQuery(self.0) };
    }
}

fn pdh_error(what: &str, status: u32) -> PlatformError {
    PlatformError::Io(format!("{what}: błąd PDH 0x{status:08X}"))
}

/// Licznik z nazwą angielską; brak licznika w systemie (np. stary sterownik) — `None`.
fn counter(query: &Query, path: PCWSTR) -> Option<PDH_HCOUNTER> {
    let mut handle = PDH_HCOUNTER::default();
    // SAFETY: ważne zapytanie, stała ścieżka zakończona zerem, bufor wyjściowy uchwytu.
    let status = unsafe { PdhAddEnglishCounterW(query.0, path, 0, &raw mut handle) };
    (status == 0).then_some(handle)
}

/// Wartości wszystkich instancji licznika.
fn values(handle: Option<PDH_HCOUNTER>, format: PDH_FMT) -> Vec<(String, f64)> {
    let Some(handle) = handle else {
        return Vec::new();
    };
    let (mut bytes, mut count) = (0u32, 0u32);
    // SAFETY: zapytanie o rozmiar (bez bufora) — oczekiwany `PDH_MORE_DATA`.
    let status = unsafe {
        PdhGetFormattedCounterArrayW(handle, format, &raw mut bytes, &raw mut count, None)
    };
    if status != PDH_MORE_DATA || bytes == 0 {
        return Vec::new();
    }
    // Bufor wyrównany do 8 bajtów: na początku tablica struktur, za nią nazwy instancji.
    let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
    let items = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
    // SAFETY: bufor ma co najmniej `bytes` bajtów i wyrównanie struktury.
    let status = unsafe {
        PdhGetFormattedCounterArrayW(handle, format, &raw mut bytes, &raw mut count, Some(items))
    };
    if status != 0 {
        return Vec::new();
    }
    // SAFETY: API zapisało `count` struktur na początku bufora, który żyje do końca funkcji.
    let rows = unsafe { std::slice::from_raw_parts(items, count as usize) };
    rows.iter()
        .filter(|item| {
            matches!(
                item.FmtValue.CStatus,
                PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
            )
        })
        .map(|item| {
            // SAFETY: nazwa wskazuje napis zakończony zerem wewnątrz bufora.
            let name = unsafe { item.szName.as_wide() };
            // SAFETY: aktywne pole unii wynika z żądanego formatu.
            let value = unsafe {
                if format == PDH_FMT_DOUBLE {
                    item.FmtValue.Anonymous.doubleValue
                } else {
                    #[allow(clippy::cast_precision_loss)]
                    let large = item.FmtValue.Anonymous.largeValue as f64;
                    large
                }
            };
            (from_wide(name), value)
        })
        .collect()
}

/// Bieżące zużycie kart: pamięć (wszystkie procesy), a z `full` także obciążenie (dwie próbki co
/// 250 ms) i procesy na kartach; bez `full` — sama pamięć jedną próbką (start silnika mowy).
pub(crate) fn usage(full: bool) -> Result<GpuUsage, PlatformError> {
    let mut raw = PDH_HQUERY::default();
    // SAFETY: zapytanie na żywych danych (bez pliku dziennika); bufor wyjściowy uchwytu.
    let status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &raw mut raw) };
    if status != 0 {
        return Err(pdh_error("PdhOpenQueryW", status));
    }
    let query = Query(raw);
    let dedicated = counter(&query, w!(r"\GPU Adapter Memory(*)\Dedicated Usage"));
    let shared = counter(&query, w!(r"\GPU Adapter Memory(*)\Shared Usage"));
    let process = full
        .then(|| counter(&query, w!(r"\GPU Process Memory(*)\Dedicated Usage")))
        .flatten();
    let engine = full
        .then(|| counter(&query, w!(r"\GPU Engine(*)\Utilization Percentage")))
        .flatten();
    if dedicated.is_none() && shared.is_none() {
        return Err(PlatformError::Unsupported(
            "liczniki „GPU Adapter Memory” niedostępne (system starszy niż Windows 10 1709?)"
                .into(),
        ));
    }
    // SAFETY: ważne zapytanie; przy pełnym odczycie dwa — licznik obciążenia potrzebuje różnicy.
    let first = unsafe { PdhCollectQueryData(query.0) };
    if first != 0 {
        return Err(pdh_error("PdhCollectQueryData", first));
    }
    if full {
        std::thread::sleep(LOAD_SAMPLE);
        // SAFETY: jw.
        let second = unsafe { PdhCollectQueryData(query.0) };
        if second != 0 {
            return Err(pdh_error("PdhCollectQueryData", second));
        }
    }
    let mut usage = assemble(&Readings {
        adapter_dedicated: values(dedicated, PDH_FMT_LARGE),
        adapter_shared: values(shared, PDH_FMT_LARGE),
        process_dedicated: values(process, PDH_FMT_LARGE),
        engine_utilization: values(engine, PDH_FMT_DOUBLE),
    });
    for p in &mut usage.processes {
        p.name = Some(crate::window::process_name(p.pid)).filter(|n| !n.is_empty());
    }
    Ok(usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luid_and_driver_are_formatted() {
        let luid = LUID {
            LowPart: 0x0000_F1A3,
            HighPart: 1,
        };
        assert_eq!(luid_u64(luid), (1 << 32) | 0xF1A3);
        // 32.0.31041.1004 (sterownik AMD Adrenalin).
        let raw = (32u64 << 48) | (31_041u64 << 16) | 1_004;
        assert_eq!(driver_text(raw), "32.0.31041.1004");
    }

    #[test]
    fn real_machine_is_read_without_errors() {
        // Na każdej maszynie z Windows: odczyt nie może się wywrócić, LUID-y są niezerowe.
        // Odczyt widać przy `cargo test -- --nocapture` (sprawdzenie na nowej maszynie).
        for d in details() {
            eprintln!("DXCore: {d:?}");
            assert_ne!(d.luid, 0);
        }
        match usage(true) {
            Ok(usage) => {
                eprintln!("PDH: {:?}", usage.adapters);
                eprintln!("PDH procesy: {}", usage.processes.len());
                assert!(usage.adapters.iter().all(|a| a.luid != 0));
            }
            Err(e) => eprintln!("PDH niedostępne: {e}"),
        }
        // Sama pamięć: jedna próbka, bez czekania na obciążenie i bez procesów.
        let started = std::time::Instant::now();
        if let Ok(memory) = usage(false) {
            eprintln!(
                "PDH pamięć: {:?} w {:?}",
                memory.adapters,
                started.elapsed()
            );
            assert!(memory.processes.is_empty());
            assert!(started.elapsed() < LOAD_SAMPLE);
        }
    }
}
