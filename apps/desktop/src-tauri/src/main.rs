// Ukrywa konsolę w buildzie release na Windows (nie dotyczy `tauri dev`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    alfa_desktop_lib::run()
}
