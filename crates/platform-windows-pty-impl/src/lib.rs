//! Pseudokonsola ConPTY (F4; docs/modules/ui-terminal/SPEC.md, docs/modules/platform-windows/SPEC.md):
//! implementacja `PseudoConsolePort` dla wbudowanego terminala logowania do CLI mostów.
//!
//! `CreatePseudoConsole` + potoki anonimowe; proces startuje wstrzymany, trafia do własnego Job
//! Object (`KILL_ON_JOB_CLOSE`), dopiero potem jest wznawiany — zamknięcie sesji zabija całe drzewo
//! (`TerminateJobObject`) i zamyka pseudokonsolę. Środowisko jawne (nic nie jest dziedziczone).
//! Crate nie loguje i nie analizuje strumienia (w terminalu logowania mogą pojawić się tokeny).
//! Poza Windows: `Unsupported`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
mod attrs;
mod cmdline;
#[cfg(windows)]
mod conpty;

use platform_contract::{PlatformError, PseudoConsolePort, PtySession, PtySpec};

pub use cmdline::{command_line, environment_block};

/// Port pseudokonsoli Windows.
#[derive(Debug, Default, Clone, Copy)]
pub struct WinPty;

impl PseudoConsolePort for WinPty {
    fn spawn(&self, spec: &PtySpec) -> Result<Box<dyn PtySession>, PlatformError> {
        spec.validate()?;
        #[cfg(windows)]
        let session = conpty::spawn(spec).map(|s| Box::new(s) as Box<dyn PtySession>);
        #[cfg(not(windows))]
        let session = Err(PlatformError::Unsupported("ConPTY: tylko Windows".into()));
        session
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use platform_contract::PtySize;

    use super::*;

    #[test]
    fn validation_happens_before_spawn() {
        let spec = PtySpec {
            program: PathBuf::from("pwsh.exe"),
            args: vec![],
            cwd: PathBuf::from("."),
            env: vec![],
            size: PtySize::default(),
        };
        assert!(matches!(
            WinPty.spawn(&spec),
            Err(PlatformError::InvalidPath(_))
        ));
    }
}
