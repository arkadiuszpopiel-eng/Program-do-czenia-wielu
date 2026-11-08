//! Pakiety „Modele lokalne i silniki” w skali ocen 1–6 (6 — wzorcowy, 1 — minimalny): dobór pozycji dla tej
//! maszyny (wersje silników CUDA / Vulkan / CPU), dopasowanie do sprzętu (pasuje / na styk / za
//! słaby), zalecany pakiet (najwyższy pasujący bez kompromisów), rozmiar i stan z pozycji.
//! Czysta logika (testy tabelaryczne); pobieranie, weryfikacja i naprawa — `bundle_ops`.

use crate::bundle_data::{DEFS, Def, Gb, Need};
pub use crate::bundle_machine::{
    Accel, GPU_MIN_MB, IGPU_MIN_RAM_MB, Machine, def, members, members_for,
};
use app_api::dto::{
    BundleFit, BundleFitKind, BundleItemView, BundleRequirements, BundleState, LocalizedText,
    ModelBundle, ModelItem, ModelItemState,
};

/// Liczba GB z jednym miejscem po przecinku (PL: przecinek, EN: kropka).
fn gb(mb: u64) -> (String, String) {
    let tenths = (mb * 10 + 512) / 1024;
    let (whole, frac) = (tenths / 10, tenths % 10);
    (format!("{whole},{frac}"), format!("{whole}.{frac}"))
}

/// „{n} rdzeń / rdzenie / rdzeni” (PL) i „{n} core(s)” (EN).
fn cores(n: u32) -> (String, String) {
    let pl = match (n % 10, n % 100) {
        _ if n == 1 => "rdzeń",
        (2..=4, r) if !(12..=14).contains(&r) => "rdzenie",
        _ => "rdzeni",
    };
    let en = if n == 1 { "core" } else { "cores" };
    (format!("{n} {pl}"), format!("{n} {en}"))
}

/// Opis wymagań (wartości nominalne).
fn requirements_text(n: &Need) -> LocalizedText {
    let ram = n.ram.nominal;
    let compute = match (n.vram, n.cores, n.gpu_required) {
        (Some(v), _, true) => Some((
            format!("karta graficzna ≥ {} GB (CUDA albo Vulkan)", v.nominal),
            format!("GPU ≥ {} GB (CUDA or Vulkan)", v.nominal),
        )),
        (Some(v), Some(c), false) => Some((
            format!(
                "procesor ≥ {c} rdzeni albo karta graficzna ≥ {} GB",
                v.nominal
            ),
            format!("CPU ≥ {c} cores or GPU ≥ {} GB", v.nominal),
        )),
        (None, Some(c), _) => Some((format!("procesor ≥ {c} rdzeni"), format!("CPU ≥ {c} cores"))),
        (Some(v), None, false) => Some((
            format!("karta graficzna ≥ {} GB", v.nominal),
            format!("GPU ≥ {} GB", v.nominal),
        )),
        (None, None, _) => None,
    };
    match compute {
        Some((pl, en)) => LocalizedText::new(
            format!("RAM ≥ {ram} GB, {pl}"),
            format!("RAM ≥ {ram} GB, {en}"),
        ),
        None => LocalizedText::new(
            format!("RAM ≥ {ram} GB (karta graficzna niepotrzebna)"),
            format!("RAM ≥ {ram} GB (no GPU needed)"),
        ),
    }
}

fn requirements(n: &Need) -> BundleRequirements {
    BundleRequirements {
        min_ram_mb: n.ram.min_mb,
        min_vram_mb: n.vram.map(|v| v.min_mb),
        min_cpu_cores: n.cores,
        gpu_required: n.gpu_required,
        text: requirements_text(n),
    }
}

/// Niespełnione wymagania (PL, EN); puste — spełnione.
fn unmet(n: &Need, m: &Machine) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if m.ram_mb < n.ram.min_mb {
        let (pl, en) = gb(m.ram_mb);
        out.push((
            format!(
                "za mało pamięci RAM: potrzeba {} GB, jest {pl} GB",
                n.ram.nominal
            ),
            format!(
                "not enough RAM: {} GB needed, {en} GB present",
                n.ram.nominal
            ),
        ));
    }
    let gpu_ok = |v: Gb| m.gpu.is_some() && m.vram_mb >= v.min_mb;
    let has = || {
        if m.gpu.is_some() {
            let (pl, en) = gb(m.vram_mb);
            (format!("karta ma {pl} GB"), format!("the GPU has {en} GB"))
        } else {
            (
                "brak karty graficznej (albo ma mniej niż 4 GB)".to_owned(),
                "no GPU (or less than 4 GB)".to_owned(),
            )
        }
    };
    match (n.vram, n.cores) {
        (Some(v), Some(c)) if !n.gpu_required => {
            if !gpu_ok(v) && m.cpu_cores < c {
                let (pl, en) = has();
                let (cpu_pl, cpu_en) = cores(m.cpu_cores);
                out.push((
                    format!(
                        "potrzebny procesor ≥ {c} rdzeni albo karta ≥ {} GB — procesor ma {cpu_pl}, {pl}",
                        v.nominal
                    ),
                    format!(
                        "a CPU ≥ {c} cores or a GPU ≥ {} GB is needed — the CPU has {cpu_en}, {en}",
                        v.nominal
                    ),
                ));
            }
        }
        (Some(v), _) => {
            if !gpu_ok(v) {
                let (pl, en) = has();
                out.push((
                    format!(
                        "potrzebna karta graficzna ≥ {} GB (CUDA albo Vulkan) — {pl}",
                        v.nominal
                    ),
                    format!("a GPU ≥ {} GB (CUDA or Vulkan) is needed — {en}", v.nominal),
                ));
            }
        }
        (None, Some(c)) => {
            if m.cpu_cores < c {
                let (cpu_pl, cpu_en) = cores(m.cpu_cores);
                out.push((
                    format!("potrzebny procesor ≥ {c} rdzeni — ma {cpu_pl}"),
                    format!("a CPU ≥ {c} cores is needed — it has {cpu_en}"),
                ));
            }
        }
        (None, None) => {}
    }
    out
}

fn sentence(parts: &[String]) -> String {
    let joined = parts.join("; ");
    let mut chars = joined.chars();
    match chars.next() {
        Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

/// Dopasowanie pakietu do maszyny.
pub fn fit(def: &Def, m: &Machine) -> BundleFit {
    let missing = unmet(&def.need, m);
    if missing.is_empty() {
        return BundleFit {
            kind: BundleFitKind::Fits,
            reason: None,
        };
    }
    if let Some(t) = def.tight.filter(|t| unmet(&t.need, m).is_empty()) {
        return BundleFit {
            kind: BundleFitKind::Tight,
            reason: Some(LocalizedText::new(t.reason.0, t.reason.1)),
        };
    }
    let (pl, en): (Vec<String>, Vec<String>) = missing.into_iter().unzip();
    BundleFit {
        kind: BundleFitKind::TooWeak,
        reason: Some(LocalizedText::new(sentence(&pl), sentence(&en))),
    }
}

fn installed(state: ModelItemState) -> bool {
    matches!(state, ModelItemState::Installed | ModelItemState::External)
}

/// Stan pakietu ze stanów pozycji.
pub fn state_of(items: &[BundleItemView]) -> BundleState {
    let any = |f: &dyn Fn(ModelItemState) -> bool| items.iter().any(|i| f(i.state));
    if any(&|s| {
        matches!(
            s,
            ModelItemState::Queued | ModelItemState::Downloading | ModelItemState::Installing
        )
    }) {
        BundleState::Downloading
    } else if any(&|s| matches!(s, ModelItemState::Corrupt | ModelItemState::Failed)) {
        BundleState::Corrupt
    } else if any(&|s| s == ModelItemState::NeedsTrust) {
        BundleState::NeedsTrust
    } else if !items.is_empty() && items.iter().all(|i| installed(i.state)) {
        BundleState::Installed
    } else if any(&|s| installed(s) || s == ModelItemState::Paused) {
        BundleState::Partial
    } else {
        BundleState::NotInstalled
    }
}

/// Bajty do pobrania pozycji (górne oszacowanie: bez pobranej części bieżącego pliku).
fn to_download(item: &ModelItem) -> u64 {
    let pending = matches!(
        item.state,
        ModelItemState::Missing
            | ModelItemState::Paused
            | ModelItemState::Failed
            | ModelItemState::Corrupt
            | ModelItemState::Queued
            | ModelItemState::Downloading
    );
    if !pending || !item.downloadable {
        return 0;
    }
    let done = item.progress.as_ref().map_or(0, |p| p.done);
    item.size_bytes.saturating_sub(done)
}

/// Widok pakietu (pozycje spoza katalogu są pomijane).
pub fn view(def: &Def, m: &Machine, recommended: bool, items: &[ModelItem]) -> ModelBundle {
    let found: Vec<(&ModelItem, bool)> = members_for(def, m)
        .into_iter()
        .filter_map(|(id, fallback)| items.iter().find(|i| i.id == id).map(|i| (i, fallback)))
        .collect();
    let rows: Vec<BundleItemView> = found
        .iter()
        .map(|(i, fallback)| BundleItemView {
            id: i.id.clone(),
            name: i.name.clone(),
            kind: i.kind,
            state: i.state,
            size_bytes: i.size_bytes,
            downloadable: i.downloadable,
            fallback: *fallback,
        })
        .collect();
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    ModelBundle {
        id: def.id.into(),
        rating: def.rating,
        name: LocalizedText::new(def.name.0, def.name.1),
        summary: LocalizedText::new(def.summary.0, def.summary.1),
        requirements: requirements(&def.need),
        size_bytes: found.iter().map(|(i, _)| i.size_bytes).sum(),
        missing_bytes: found.iter().map(|(i, _)| to_download(i)).sum(),
        installed: count(rows.iter().filter(|v| installed(v.state)).count()),
        total: count(rows.len()),
        state: state_of(&rows),
        fit: fit(def, m),
        recommended,
        quality: (def.quality)(),
        items: rows,
    }
}

/// Zalecany pakiet: najwyższa ocena, która pasuje bez kompromisów (`None` — żaden).
pub fn recommended(m: &Machine) -> Option<u8> {
    DEFS.iter()
        .filter(|d| fit(d, m).kind == BundleFitKind::Fits)
        .map(|d| d.rating)
        .max()
}

/// Pakiety od 6 do 1 dla maszyny i stanów pozycji (`items` — widok `models_list`).
pub fn views(m: &Machine, items: &[ModelItem]) -> Vec<ModelBundle> {
    let best = recommended(m);
    DEFS.iter()
        .map(|d| view(d, m, best == Some(d.rating), items))
        .collect()
}
