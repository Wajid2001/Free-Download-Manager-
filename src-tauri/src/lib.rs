mod downloads;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(downloads::DownloadManager::new())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            downloads::list_downloads,
            downloads::set_speed_limits,
            downloads::start_download,
            downloads::pause_download,
            downloads::resume_download,
            downloads::cancel_download,
            downloads::restart_download,
            downloads::remove_download,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
