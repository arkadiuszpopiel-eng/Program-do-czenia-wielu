//! Stały launcher `%LOCALAPPDATA%\Alfa\alfa.exe` (ADR 0007): czyta `current.json`, uruchamia
//! `versions\<ver>\alfa-desktop.exe` z przekazaniem argumentów (URI `alfa://`, ścieżki „Otwórz
//! w Alfie”/„Wyślij do”), przy crash-loopie wraca do poprzedniej wersji. Bez okna konsoli.

#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    updater_impl::launcher::main_entry()
}
