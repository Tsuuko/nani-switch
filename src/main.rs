#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod crypto;
mod db;
mod dialog;
mod logger;
mod model;
mod nani;
mod paths;
mod startup;
mod store;
mod usage;

fn main() {
    let instance = match single_instance::SingleInstance::new("nani-switch-rust-tray") {
        Ok(instance) => instance,
        Err(error) => {
            dialog::error("Nani Switch failed", &error.to_string());
            return;
        }
    };
    if !instance.is_single() {
        return;
    }
    if let Err(error) = app::run() {
        logger::error(format!("Nani Switch failed: {error:#}"));
        dialog::error("Nani Switch failed", &format!("{error:#}"));
    }
}
