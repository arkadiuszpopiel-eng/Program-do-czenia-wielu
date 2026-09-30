//! Win32 sondy sprzętu. DXGI i DXCore nie wymagają `CoInitialize`; MMDevice idzie na wątek MTA.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::mem::size_of;

use platform_contract::{
    AudioDirection, AudioEndpoint, CpuSummary, GpuAdapter, OsSummary, PlatformError, PowerStatus,
};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Graphics::DXCore::{
    DXCORE_ADAPTER_ATTRIBUTE_D3D12_GRAPHICS, DXCoreCreateAdapterFactory, DriverDescription,
    IDXCoreAdapter, IDXCoreAdapterFactory, IDXCoreAdapterList, IsHardware,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIFactory1,
};
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, IMMDeviceEnumerator, IMMEndpoint, MMDeviceEnumerator, eAll, eCapture,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, STGM_READ};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegGetValueW,
};
use windows::Win32::System::SystemInformation::{
    GetLogicalProcessorInformation, GetPhysicallyInstalledSystemMemory, GlobalMemoryStatusEx,
    MEMORYSTATUSEX, RelationCache, RelationProcessorCore, SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
};
use windows::core::{GUID, Interface, PCWSTR, w};

use super::windows_name;
use crate::win::{Apartment, from_wide, run_in_apartment, win_error};

/// `DXCORE_ADAPTER_ATTRIBUTE_D3D12_GENERIC_ML` (dxcore_interface.h; brak w windows-rs 0.62).
const DXCORE_GENERIC_ML: GUID = GUID::from_u128(0xb71b0d41_1088_422f_a27c_0250b7d3a988);
const MB: u64 = 1024 * 1024;

fn reg_sz(subkey: PCWSTR, value: PCWSTR) -> Option<String> {
    let flags = RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY;
    let mut size = 0u32;
    // SAFETY: zapytanie o rozmiar (bez bufora); napisy stałe zakończone zerem.
    let probe = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey,
            value,
            flags,
            None,
            None,
            Some(&raw mut size),
        )
    };
    if probe.is_err() || size == 0 {
        return None;
    }
    let mut buffer = vec![0u16; (size as usize).div_ceil(2) + 1];
    let mut bytes = u32::try_from(buffer.len() * 2).ok()?;
    // SAFETY: bufor ma `bytes` bajtów; API zapisuje napis zakończony zerem.
    let read = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            subkey,
            value,
            flags,
            None,
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            Some(&raw mut bytes),
        )
    };
    read.is_ok().then(|| from_wide(&buffer).trim().to_owned())
}

pub(crate) fn os() -> Result<OsSummary, PlatformError> {
    let key = w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let build = reg_sz(key, w!("CurrentBuild")).and_then(|b| b.parse::<u32>().ok());
    // `ProductName` na Windows 11 nadal mówi „Windows 10”; wiarygodny jest tylko dla serwerów.
    let product = reg_sz(key, w!("ProductName")).filter(|p| p.contains("Server"));
    Ok(OsSummary {
        name: product.unwrap_or_else(|| windows_name(build).to_owned()),
        version: reg_sz(key, w!("DisplayVersion")).unwrap_or_default(),
        build,
    })
}

pub(crate) fn cpu() -> Result<CpuSummary, PlatformError> {
    let model = reg_sz(
        w!(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"),
        w!("ProcessorNameString"),
    )
    .unwrap_or_default();
    let mut bytes = 0u32;
    // SAFETY: zapytanie o wymagany rozmiar (oczekiwany błąd ERROR_INSUFFICIENT_BUFFER).
    let _ = unsafe { GetLogicalProcessorInformation(None, &raw mut bytes) };
    let count = bytes as usize / size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>();
    let mut items = vec![SYSTEM_LOGICAL_PROCESSOR_INFORMATION::default(); count.max(1)];
    // SAFETY: tablica ma co najmniej `bytes` bajtów.
    unsafe { GetLogicalProcessorInformation(Some(items.as_mut_ptr()), &raw mut bytes) }
        .map_err(|e| win_error("GetLogicalProcessorInformation", &e))?;
    items.truncate(bytes as usize / size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>());
    let (mut physical, mut logical, mut l3_bytes) = (0u32, 0u32, 0u64);
    for item in &items {
        if item.Relationship == RelationProcessorCore {
            physical += 1;
            logical += item.ProcessorMask.count_ones();
        } else if item.Relationship == RelationCache {
            // SAFETY: dla `RelationCache` aktywnym polem unii jest `Cache`.
            let cache = unsafe { item.Anonymous.Cache };
            if cache.Level == 3 {
                l3_bytes += u64::from(cache.Size);
            }
        }
    }
    Ok(CpuSummary {
        model,
        physical_cores: physical,
        logical_cores: logical,
        l3_cache_kb: (l3_bytes > 0).then(|| u32::try_from(l3_bytes / 1024).unwrap_or(u32::MAX)),
    })
}

pub(crate) fn memory_total_mb() -> Result<u64, PlatformError> {
    let mut kb = 0u64;
    // SAFETY: bufor wyjściowy `u64`.
    if unsafe { GetPhysicallyInstalledSystemMemory(&raw mut kb) }.is_ok() && kb > 0 {
        return Ok(kb / 1024);
    }
    let mut status = MEMORYSTATUSEX {
        dwLength: u32::try_from(size_of::<MEMORYSTATUSEX>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: `dwLength` ustawione zgodnie z wymogiem API.
    unsafe { GlobalMemoryStatusEx(&raw mut status) }
        .map_err(|e| win_error("GlobalMemoryStatusEx", &e))?;
    Ok(status.ullTotalPhys / MB)
}

pub(crate) fn gpus() -> Result<Vec<GpuAdapter>, PlatformError> {
    // SAFETY: fabryka DXGI 1.1 (bez inicjalizacji COM).
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.map_err(|e| win_error("CreateDXGIFactory1", &e))?;
    let mut out = Vec::new();
    for index in 0.. {
        // SAFETY: kolejne adaptery aż do DXGI_ERROR_NOT_FOUND.
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(e) => return Err(win_error("IDXGIFactory1::EnumAdapters1", &e)),
        };
        // SAFETY: ważny adapter z fabryki.
        let desc = unsafe { adapter.GetDesc1() }.map_err(|e| win_error("GetDesc1", &e))?;
        out.push(GpuAdapter {
            name: from_wide(&desc.Description),
            vendor_id: desc.VendorId,
            device_id: desc.DeviceId,
            dedicated_vram_mb: desc.DedicatedVideoMemory as u64 / MB,
            shared_memory_mb: desc.SharedSystemMemory as u64 / MB,
            software: desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0,
        });
    }
    Ok(out)
}

/// NPU = sprzętowy adapter DXCore z atrybutem „generic ML” bez grafiki D3D12. Starsze systemy
/// (bez tego atrybutu) zwracają pustą listę → `None`.
pub(crate) fn npu() -> Result<Option<String>, PlatformError> {
    // SAFETY: fabryka DXCore (bez inicjalizacji COM); brak dxcore.dll → błąd → brak NPU.
    let Ok(factory) = (unsafe { DXCoreCreateAdapterFactory::<IDXCoreAdapterFactory>() }) else {
        return Ok(None);
    };
    // SAFETY: lista adapterów z filtrem atrybutu (stały GUID).
    let Ok(list) =
        (unsafe { factory.CreateAdapterList::<IDXCoreAdapterList>(&[DXCORE_GENERIC_ML]) })
    else {
        return Ok(None);
    };
    // SAFETY: ważna lista.
    let count = unsafe { list.GetAdapterCount() };
    for index in 0..count {
        // SAFETY: indeks z zakresu listy.
        let Ok(adapter) = (unsafe { list.GetAdapter::<IDXCoreAdapter>(index) }) else {
            continue;
        };
        // SAFETY: zapytania o atrybuty/właściwości ważnego adaptera.
        let graphics =
            unsafe { adapter.IsAttributeSupported(&DXCORE_ADAPTER_ATTRIBUTE_D3D12_GRAPHICS) };
        let mut hardware = 0u8;
        // SAFETY: `IsHardware` to jednobajtowa wartość logiczna.
        let hw_ok =
            unsafe { adapter.GetProperty(IsHardware, 1, (&raw mut hardware).cast::<c_void>()) };
        if graphics || hw_ok.is_err() || hardware == 0 {
            continue;
        }
        // SAFETY: rozmiar opisu sterownika, potem odczyt do bufora tego rozmiaru.
        let size = unsafe { adapter.GetPropertySize(DriverDescription) }.unwrap_or(0);
        let mut name = vec![0u8; size.max(1)];
        // SAFETY: jw.
        let read = unsafe {
            adapter.GetProperty(
                DriverDescription,
                name.len(),
                name.as_mut_ptr().cast::<c_void>(),
            )
        };
        let text = read.ok().map(|()| {
            let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
            String::from_utf8_lossy(&name[..end]).trim().to_owned()
        });
        return Ok(Some(
            text.filter(|t| !t.is_empty())
                .unwrap_or_else(|| "NPU".into()),
        ));
    }
    Ok(None)
}

pub(crate) fn power_status() -> Result<PowerStatus, PlatformError> {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: bufor wyjściowy.
    unsafe { GetSystemPowerStatus(&raw mut status) }
        .map_err(|e| win_error("GetSystemPowerStatus", &e))?;
    // BatteryFlag: 128 = brak baterii, 255 = nieznane.
    let battery_present = status.BatteryFlag != 128 && status.BatteryFlag != 255;
    Ok(PowerStatus {
        ac_online: match status.ACLineStatus {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        battery_present,
        battery_percent: (battery_present && status.BatteryLifePercent <= 100)
            .then_some(status.BatteryLifePercent),
    })
}

fn endpoints_on_mta() -> Result<Vec<AudioEndpoint>, PlatformError> {
    let err = |ctx: &'static str| move |e: windows::core::Error| win_error(ctx, &e);
    // SAFETY: COM zainicjalizowany jako MTA przez `run_in_apartment`.
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(err("CoCreateInstance(MMDeviceEnumerator)"))?;
    // SAFETY: ważny enumerator.
    let devices = unsafe { enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE) }
        .map_err(err("EnumAudioEndpoints"))?;
    // SAFETY: ważna kolekcja.
    let count = unsafe { devices.GetCount() }.map_err(err("IMMDeviceCollection::GetCount"))?;
    let mut out = Vec::new();
    for index in 0..count {
        // SAFETY: indeks z zakresu kolekcji; obiekty zwalniane przez `Drop`.
        let Ok(device) = (unsafe { devices.Item(index) }) else {
            continue;
        };
        // SAFETY: odczyt magazynu właściwości urządzenia.
        let Ok(store) = (unsafe { device.OpenPropertyStore(STGM_READ) }) else {
            continue;
        };
        // SAFETY: stały klucz właściwości; PROPVARIANT czyszczony przez `Drop`.
        let name = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }
            .map(|v| v.to_string())
            .unwrap_or_default();
        let flow = device
            .cast::<IMMEndpoint>()
            .ok()
            // SAFETY: ważny endpoint.
            .and_then(|e| unsafe { e.GetDataFlow() }.ok());
        out.push(AudioEndpoint {
            name,
            direction: if flow == Some(eCapture) {
                AudioDirection::Capture
            } else {
                AudioDirection::Render
            },
        });
    }
    Ok(out)
}

pub(crate) fn audio_endpoints() -> Result<Vec<AudioEndpoint>, PlatformError> {
    run_in_apartment(Apartment::Mta, endpoints_on_mta)
}

pub(crate) fn machine_guid() -> Result<Option<String>, PlatformError> {
    Ok(reg_sz(
        w!(r"SOFTWARE\Microsoft\Cryptography"),
        w!("MachineGuid"),
    ))
}
