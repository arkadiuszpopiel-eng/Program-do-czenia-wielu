//! Poza Windows: porty Jądra niedostępne (`Unsupported`) — testy logiki używają `platform-fake`.

use std::path::Path;

use platform_contract::{
    DiskSpace, MmcssTask, PeerIdentity, PipeConnection, PipeListener, PipeSecurity, PlatformError,
    ServiceBody, SessionLaunch, Sid, ThreadBoost,
};

/// Uchwyt procesu (brak poza Windows).
#[derive(Debug)]
pub(crate) struct Proc;

macro_rules! unsupported {
    ($($name:ident($($arg:ty),*) -> $ret:ty, $what:literal;)*) => {$(
        pub(crate) fn $name($(_: $arg),*) -> Result<$ret, PlatformError> {
            Err(PlatformError::Unsupported(format!("{}: tylko Windows", $what)))
        }
    )*};
}

unsupported! {
    listen(&PipeSecurity) -> Box<dyn PipeListener>, "named pipe";
    connect(&str, u32) -> Box<dyn PipeConnection>, "named pipe";
    identify(u32) -> PeerIdentity, "tożsamość procesu";
    current_user() -> Sid, "SID bieżącego konta";
    ensure_private_dir(&Path, &Sid) -> (), "katalog z DACL";
    mmcss_boost(MmcssTask) -> ThreadBoost, "MMCSS";
    free_disk_space(&Path) -> DiskSpace, "wolne miejsce";
    run_service(&str, ServiceBody) -> (), "usługa Windows";
    launch_high(&SessionLaunch) -> (u32, Proc), "uruchomienie z wysoką integralnością";
}

pub(crate) fn proc_running(_: &Proc) -> bool {
    false
}
