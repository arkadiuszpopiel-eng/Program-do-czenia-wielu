// Bez okna konsoli na Windows: build release i kopia właściciela (cecha `no-console`, `deploy-owner.sh`);
// build testowy i `tauri dev` zostają z konsolą (dziennik na stderr).
#![cfg_attr(
    any(not(debug_assertions), feature = "no-console"),
    windows_subsystem = "windows"
)]

fn main() {
    alfa_desktop_lib::run()
}
