//! `alfa-broker` — usługa Brokera (Safety Kernel). `alfa-broker --config <plik.json>` jako usługa
//! Windows `AlfaBroker`; `alfa-broker --console [--config <plik.json>]` — tryb deweloperski.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use app_safety::broker::{SERVICE_NAME, dev_config, load_config, run};
use app_safety::{arg_value, has_flag};
use platform_contract::{ProcessIdentityPort, ServiceHostPort, StopSignal};
use platform_windows_kernel_impl::WinKernel;
use safety_broker_impl::service::ServiceConfig;

fn console_config(config: Option<String>) -> Result<ServiceConfig, String> {
    if let Some(path) = config {
        return load_config(Path::new(&path));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let profile = std::env::var("USERPROFILE").map_err(|_| "brak USERPROFILE".to_owned())?;
    let local = std::env::var("LOCALAPPDATA").map_err(|_| "brak LOCALAPPDATA".to_owned())?;
    let user = WinKernel.current_user().map_err(|e| e.to_string())?;
    let data = PathBuf::from(local).join("Alfa").join("broker-dev");
    Ok(dev_config(&dir, user, &profile, data))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = arg_value(&args, "--config");
    let result = if has_flag(&args, "--console") {
        console_config(config).and_then(|c| run(c, &StopSignal::new()))
    } else {
        match config.map(|p| load_config(Path::new(&p))) {
            Some(Ok(c)) => WinKernel
                .run_service(SERVICE_NAME, Box::new(move |stop| run(c, &stop)))
                .map_err(|e| e.to_string()),
            Some(Err(e)) => Err(e),
            None => Err("użycie: alfa-broker --config <plik.json> | --console".into()),
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[alfa-broker] {e}");
            ExitCode::FAILURE
        }
    }
}
