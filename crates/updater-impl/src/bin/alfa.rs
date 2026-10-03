//! Stały launcher `%LOCALAPPDATA%\Alfa\alfa.exe` (ADR 0007): czyta `current.json`, uruchamia
//! `versions\<ver>\alfa-desktop.exe` z przekazaniem argumentów (URI `alfa://`, ścieżki „Otwórz
//! w Alfie”/„Wyślij do”), przy crash-loopie albo braku `mark_good` wraca do poprzedniej wersji,
//! zamienia przygotowany nowy launcher; tryby `--alfa-restart`, `--alfa-installed <ver>`,
//! `--alfa-launcher-check` (`updater_impl::entry`). Bez okna konsoli.

#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    updater_impl::entry::main_entry()
}
