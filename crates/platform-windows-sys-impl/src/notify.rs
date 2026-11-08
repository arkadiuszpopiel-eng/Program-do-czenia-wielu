//! Bufor `ReadDirectoryChangesW` (łańcuch `FILE_NOTIFY_INFORMATION`) → zmiany surowe kontraktu.
//! Bez `unsafe` i bez Windows (testowalne na Linuksie): rekordy czytane po bajtach, uszkodzony
//! bufor kończy parsowanie (nigdy odczyt poza bufor), para nazw przemianowania łączona także
//! między kolejnymi buforami; katalog dodany, przeniesiony albo usunięty → pełne przeskanowanie.

use std::path::{Path, PathBuf};

use platform_contract::RawChange;

use crate::scan::Stat;

/// Akcja z `FILE_NOTIFY_INFORMATION::Action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyAction {
    /// `FILE_ACTION_ADDED` (1).
    Added,
    /// `FILE_ACTION_REMOVED` (2).
    Removed,
    /// `FILE_ACTION_MODIFIED` (3).
    Modified,
    /// `FILE_ACTION_RENAMED_OLD_NAME` (4).
    RenamedOld,
    /// `FILE_ACTION_RENAMED_NEW_NAME` (5).
    RenamedNew,
    /// Inna wartość.
    Other(u32),
}

impl NotifyAction {
    /// Z wartości surowej.
    pub fn from_raw(value: u32) -> Self {
        match value {
            1 => Self::Added,
            2 => Self::Removed,
            3 => Self::Modified,
            4 => Self::RenamedOld,
            5 => Self::RenamedNew,
            v => Self::Other(v),
        }
    }
}

fn u32_at(buf: &[u8], offset: usize) -> Option<u32> {
    let b = buf.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Rekordy bufora: `NextEntryOffset`, `Action`, `FileNameLength` (bajty), nazwa UTF-16 (ścieżka
/// względna wobec obserwowanego katalogu, bez zera na końcu).
pub fn parse_notify_buffer(buf: &[u8]) -> Vec<(NotifyAction, String)> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    loop {
        let (Some(next), Some(action), Some(len)) = (
            u32_at(buf, offset),
            u32_at(buf, offset + 4),
            u32_at(buf, offset + 8),
        ) else {
            break;
        };
        let start = offset + 12;
        let Some(name) = usize::try_from(len)
            .ok()
            .filter(|l| l % 2 == 0)
            .and_then(|l| buf.get(start..start.checked_add(l)?))
        else {
            break;
        };
        let units: Vec<u16> = name
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        out.push((
            NotifyAction::from_raw(action),
            String::from_utf16_lossy(&units),
        ));
        let Ok(next) = usize::try_from(next) else {
            break;
        };
        if next == 0 {
            break;
        }
        match offset.checked_add(next) {
            Some(n) if n > offset => offset = n,
            _ => break,
        }
    }
    out
}

/// Tłumacz rekordów na zmiany surowe (stan: niesparowana stara nazwa przemianowania).
#[derive(Debug, Default)]
pub(crate) struct Translator {
    old: Option<PathBuf>,
}

/// Wynik tłumaczenia jednego bufora.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Translated {
    pub(crate) changes: Vec<RawChange>,
    pub(crate) rescan: bool,
}

impl Translator {
    /// `stat` — stan ścieżki teraz; `knows_under` — czy obserwacja zna pliki pod ścieżką
    /// (usunięty/przeniesiony katalog).
    pub(crate) fn translate(
        &mut self,
        dir: &Path,
        entries: Vec<(NotifyAction, String)>,
        stat: impl Fn(&Path) -> Stat,
        knows_under: impl Fn(&Path) -> bool,
    ) -> Translated {
        let mut out = Translated::default();
        for (action, name) in entries {
            let path = dir.join(name);
            match action {
                NotifyAction::Added => self.appeared(&mut out, path, &stat),
                NotifyAction::Modified => {
                    if let Stat::File(s) = stat(&path) {
                        out.changes.push(RawChange::Modified(path, s));
                    }
                }
                NotifyAction::Removed => {
                    out.rescan |= knows_under(&path);
                    out.changes.push(RawChange::Removed(path));
                }
                NotifyAction::RenamedOld => {
                    if let Some(prev) = self.old.replace(path) {
                        out.rescan |= knows_under(&prev);
                        out.changes.push(RawChange::Removed(prev));
                    }
                }
                NotifyAction::RenamedNew => match self.old.take() {
                    Some(from) => {
                        out.rescan |= knows_under(&from);
                        match stat(&path) {
                            Stat::File(s) => out.changes.push(RawChange::Renamed {
                                from,
                                to: path,
                                stamp: s,
                            }),
                            Stat::Dir => out.rescan = true,
                            Stat::Missing => out.changes.push(RawChange::Removed(from)),
                        }
                    }
                    None => self.appeared(&mut out, path, &stat),
                },
                NotifyAction::Other(_) => {}
            }
        }
        out
    }

    fn appeared(&self, out: &mut Translated, path: PathBuf, stat: &impl Fn(&Path) -> Stat) {
        match stat(&path) {
            Stat::File(s) => out.changes.push(RawChange::Added(path, s)),
            Stat::Dir => out.rescan = true,
            Stat::Missing => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use platform_contract::FileStamp;

    use super::*;

    fn record(next: u32, action: u32, name: &str) -> Vec<u8> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut out = Vec::new();
        out.extend(next.to_le_bytes());
        out.extend(action.to_le_bytes());
        out.extend(u32::try_from(units.len() * 2).unwrap().to_le_bytes());
        for u in units {
            out.extend(u.to_le_bytes());
        }
        while out.len() % 4 != 0 {
            out.push(0);
        }
        out
    }

    fn chain(records: &[(u32, &str)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (i, (action, name)) in records.iter().enumerate() {
            let rec = record(0, *action, name);
            let next = if i + 1 == records.len() {
                0
            } else {
                u32::try_from(rec.len()).unwrap()
            };
            let start = out.len();
            out.extend(rec);
            out[start..start + 4].copy_from_slice(&next.to_le_bytes());
        }
        out
    }

    #[test]
    fn parses_chain_and_survives_garbage() {
        let buf = chain(&[(3, "żółw.txt"), (5, "sub\\nowy.txt"), (1, "faktura.pdf")]);
        let parsed = parse_notify_buffer(&buf);
        assert_eq!(
            parsed,
            vec![
                (NotifyAction::Modified, "żółw.txt".into()),
                (NotifyAction::RenamedNew, "sub\\nowy.txt".into()),
                (NotifyAction::Added, "faktura.pdf".into()),
            ]
        );
        // Uszkodzone: długość nazwy poza buforem, nieparzysta, przesunięcie w tył.
        assert!(parse_notify_buffer(&[1, 2, 3]).is_empty());
        let mut bad = record(0, 1, "x");
        bad[8] = 200;
        assert!(parse_notify_buffer(&bad).is_empty());
        bad[8] = 1;
        assert!(parse_notify_buffer(&bad).is_empty());
        let mut loop_back = record(0, 2, "y");
        loop_back[0] = 0xFF;
        loop_back[1] = 0xFF;
        loop_back[2] = 0xFF;
        loop_back[3] = 0xFF;
        assert_eq!(parse_notify_buffer(&loop_back).len(), 1);
        assert_eq!(NotifyAction::from_raw(9), NotifyAction::Other(9));
    }

    #[test]
    fn translates_renames_dirs_and_missing_files() {
        let dir = Path::new("/w");
        let stamp = FileStamp {
            len: 1,
            modified_ms: 2,
        };
        let stat = |p: &Path| match p.file_name().and_then(|n| n.to_str()) {
            Some("katalog") => Stat::Dir,
            Some("zniknal") => Stat::Missing,
            _ => Stat::File(stamp),
        };
        let mut t = Translator::default();
        let out = t.translate(
            dir,
            vec![
                (NotifyAction::Added, "a".into()),
                (NotifyAction::Added, "zniknal".into()),
                (NotifyAction::Modified, "katalog".into()),
                (NotifyAction::RenamedOld, "b".into()),
            ],
            stat,
            |_| false,
        );
        assert_eq!(out.changes, vec![RawChange::Added(dir.join("a"), stamp)]);
        assert!(!out.rescan);
        // Druga połowa pary w następnym buforze.
        let out = t.translate(
            dir,
            vec![(NotifyAction::RenamedNew, "c".into())],
            stat,
            |_| false,
        );
        assert_eq!(
            out.changes,
            vec![RawChange::Renamed {
                from: dir.join("b"),
                to: dir.join("c"),
                stamp
            }]
        );
        let out = t.translate(
            dir,
            vec![
                (NotifyAction::Added, "katalog".into()),
                (NotifyAction::Removed, "stary-katalog".into()),
            ],
            stat,
            |p| p.ends_with("stary-katalog"),
        );
        assert!(out.rescan);
        let out = t.translate(
            dir,
            vec![
                (NotifyAction::RenamedOld, "x".into()),
                (NotifyAction::RenamedOld, "y".into()),
                (NotifyAction::RenamedNew, "zniknal".into()),
                (NotifyAction::RenamedNew, "z".into()),
            ],
            stat,
            |_| false,
        );
        assert_eq!(
            out.changes,
            vec![
                RawChange::Removed(dir.join("x")),
                RawChange::Removed(dir.join("y")),
                RawChange::Added(dir.join("z"), stamp),
            ]
        );
    }
}
