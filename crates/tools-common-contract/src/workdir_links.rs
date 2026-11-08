//! Dowiązania w folderze pracy (plan 50 §8, luka 4 z przeglądu 3.11). Sędzia ocenia „w folderze
//! pracy” leksykalnie, więc junction albo symlink w folderze pracy prowadzący poza niego dawał
//! zapis bez pytania w trybie „Akceptuj edycje”. Narzędzie sprawdza ścieżkę po rozwiązaniu
//! dowiązań przy każdym wywołaniu; gdy wychodzi poza (rozwiązany) folder pracy, nie podaje
//! Sędziemu folderu pracy — działanie jest wtedy „poza folderem” i tryb pyta.

use compliance_contract::PathEnv;
use safety_broker_contract::PathScope;

use crate::paths::resolve_links;

/// Czy ścieżka leżąca leksykalnie w folderze pracy prowadzi przez dowiązanie poza niego. Ścieżka
/// już poza folderem albo brak folderu — `false` (Sędzia i tak traktuje ją jako „poza”);
/// dowiązanie nierozwiązywalne (zerwane, brak dostępu) — `true` (ostrożnie).
#[must_use]
pub fn escapes_workdir(path: &str, workdir: Option<&str>, env: &PathEnv) -> bool {
    let Some(workdir) = workdir.filter(|w| !w.trim().is_empty()) else {
        return false;
    };
    let (Ok(target), Ok(dir)) = (PathScope::exact(path, env), PathScope::tree(workdir, env)) else {
        return false;
    };
    if !target.is_subset_of(&dir) {
        return false;
    }
    let real_dir = match resolve_links(dir.canonical()) {
        Ok(Some(d)) => d,
        Ok(None) => return false,
        Err(_) => return true,
    };
    let real = match resolve_links(target.canonical()) {
        Ok(Some(r)) => r,
        Ok(None) => return false,
        Err(_) => return true,
    };
    match (
        PathScope::exact(&real, env),
        PathScope::tree(&real_dir, env),
    ) {
        (Ok(real), Ok(real_dir)) => !real.is_subset_of(&real_dir),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_paths_do_not_escape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wd = dir.path().to_string_lossy().into_owned();
        let env = PathEnv::new();
        let inside = dir.path().join("nowy").join("plik.txt");
        assert!(!escapes_workdir(&inside.to_string_lossy(), Some(&wd), &env));
        assert!(!escapes_workdir(&inside.to_string_lossy(), None, &env));
        let outside = std::env::temp_dir().join("poza.txt");
        assert!(!escapes_workdir(
            &outside.to_string_lossy(),
            Some(&wd),
            &env
        ));
    }

    #[cfg(windows)]
    #[test]
    fn junction_inside_the_workdir_escapes() {
        let root = tempfile::tempdir().expect("tempdir");
        let wd = root.path().join("projekt");
        let away = root.path().join("dane");
        std::fs::create_dir_all(&wd).expect("projekt");
        std::fs::create_dir_all(&away).expect("dane");
        let link = wd.join("skrot");
        let system = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let status = std::process::Command::new(format!(r"{system}\System32\cmd.exe"))
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&away)
            .stdout(std::process::Stdio::null())
            .status()
            .expect("mklink");
        assert!(status.success());
        let env = PathEnv::new();
        let wd_text = wd.to_string_lossy().into_owned();
        let through = link.join("raport.txt");
        assert!(escapes_workdir(
            &through.to_string_lossy(),
            Some(&wd_text),
            &env
        ));
        let plain = wd.join("raport.txt");
        assert!(!escapes_workdir(
            &plain.to_string_lossy(),
            Some(&wd_text),
            &env
        ));
        // Folder pracy sam jako dowiązanie — ścieżki w nim nie „uciekają”.
        let alias = root.path().join("alias");
        let status = std::process::Command::new(format!(r"{system}\System32\cmd.exe"))
            .args(["/C", "mklink", "/J"])
            .arg(&alias)
            .arg(&wd)
            .stdout(std::process::Stdio::null())
            .status()
            .expect("mklink");
        assert!(status.success());
        let alias_text = alias.to_string_lossy().into_owned();
        let inside = alias.join("raport.txt");
        assert!(!escapes_workdir(
            &inside.to_string_lossy(),
            Some(&alias_text),
            &env
        ));
        assert!(escapes_workdir(
            &alias.join("skrot").join("x.txt").to_string_lossy(),
            Some(&alias_text),
            &env
        ));
    }
}
