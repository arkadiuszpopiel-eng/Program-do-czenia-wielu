//! Ścieżki sieciowe w argumentach narzędzi (przegląd fali 3, W3-03): udział UNC
//! (`\\serwer\udział`, `//serwer/udział`) i WebDAV (`\\host@SSL\DavWWWRoot\…`). Sam dostęp do
//! takiej ścieżki — także `symlink_metadata`/`canonicalize` przy sprawdzaniu dowiązań, jeszcze
//! przed Brokerem — łączy się z innym komputerem (SMB/WebDAV) i uwierzytelnia kontem właściciela
//! (NTLM): to kanał wyjścia (THREAT_MODEL §5 „zapis do udziału”) i wyciek skrótu hasła. Dozwolone
//! tylko wewnątrz sieciowego katalogu roboczego sesji, który wybrał właściciel.

/// Czy ścieżka jest sieciowa (dwa separatory na początku).
pub fn is_network_path(path: &str) -> bool {
    let mut c = path.chars();
    matches!((c.next(), c.next()), (Some('/' | '\\'), Some('/' | '\\')))
}

/// Czy ścieżkę sieciową `full` wolno użyć: leży w sieciowym katalogu roboczym `workdir`.
pub(crate) fn allowed(full: &str, workdir: Option<&str>) -> bool {
    let norm = |p: &str| p.replace('/', "\\").trim_end_matches('\\').to_lowercase();
    workdir.filter(|w| is_network_path(w)).is_some_and(|w| {
        let (w, f) = (norm(w), norm(full));
        f == w || f.starts_with(&format!("{w}\\"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::{PathError, resolve_path, resolve_path_dots};
    use compliance_contract::PathEnv;

    #[test]
    fn network_paths_only_inside_a_network_workdir() {
        let env = PathEnv::windows_profile(r"C:\Users\ala");
        for raw in [
            r"\\napastnik\udzial\x.txt",
            r"\\napastnik@SSL\DavWWWRoot\x.txt",
            "//napastnik/udzial/x.txt",
            r"\/napastnik\udzial\x.txt",
        ] {
            assert!(is_network_path(raw), "{raw}");
            assert!(
                matches!(resolve_path(raw, None, &env), Err(PathError::Network(_))),
                "{raw}"
            );
            assert!(
                matches!(
                    resolve_path(raw, Some(r"C:\Users\ala\Praca"), &env),
                    Err(PathError::Network(_))
                ),
                "{raw}"
            );
        }
        let nas = r"\\nas\dom\Praca";
        assert_eq!(
            resolve_path("a.txt", Some(nas), &env).unwrap(),
            r"\\nas\dom\Praca\a.txt"
        );
        assert!(resolve_path(r"\\NAS\dom\praca\b\c.txt", Some(nas), &env).is_ok());
        for outside in [
            r"\\nas\dom\Inne\x",
            r"\\nas\dom\Praca2\x",
            r"\\inny\dom\Praca\x",
        ] {
            assert!(
                matches!(
                    resolve_path(outside, Some(nas), &env),
                    Err(PathError::Network(_))
                ),
                "{outside}"
            );
        }
        assert!(resolve_path_dots(r"..\Inne\x", Some(nas), &env).is_err());
        assert!(!is_network_path(r"C:\x") && !is_network_path("/Users/ala/x"));
        assert!(resolve_path("/Users/ala/x", None, &env).is_ok());
    }
}
