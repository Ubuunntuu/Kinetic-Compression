#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = kinetic_desktop_lib::shell::try_handle_cli_invocation(&args) {
        std::process::exit(code);
    }
    kinetic_desktop_lib::run();
}