#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

fn main() {
    tauri::Builder::default()
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::scan,
            commands::get_programs,
            commands::quarantine_items,
            commands::list_quarantine,
            commands::restore_entry,
            commands::delete_entry,
            commands::clear_quarantine,
            commands::uninstall_programs,
            commands::find_leftovers_cmd,
            commands::analyze_disk,
            commands::list_drives,
            commands::open_url
        ])
        .run(tauri::generate_context!())
        .expect("error running Svalka");
}
