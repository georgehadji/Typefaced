//! Binary entry point of the Typefaced desktop app: starts [`desktop_lib::run`].

// Prevents an extra console window on Windows in release builds. Do not remove.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    desktop_lib::run()
}
