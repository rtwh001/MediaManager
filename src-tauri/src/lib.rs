mod anilist;
mod bangumi;
mod database;
mod diagnostics;
mod parser;
mod probe;
mod scanner;
mod scraper;
mod tmdb;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            probe::initialize(app.handle());
            let database = database::initialize(app.handle())?;
            log::info!("MediaManager initialized");
            app.manage(database);
            app.manage(scanner::ScannerState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            database::database_status,
            database::export_library_backup,
            database::restore_library_backup,
            database::migrate_media_paths,
            database::list_scan_sources,
            database::list_media_items,
            database::update_media_item,
            database::set_watched_status,
            database::set_media_type,
            database::delete_media_items,
            database::merge_media_items,
            database::list_blacklist,
            database::restore_blacklist_items,
            database::clear_blacklist,
            database::set_media_poster,
            database::list_tags,
            database::create_tag,
            database::set_media_tags,
            database::list_collections,
            database::create_collection,
            database::set_media_collections,
            database::list_scan_history,
            database::merge_duplicate_media,
            database::add_scan_source,
            database::remove_scan_source,
            diagnostics::diagnostics_report,
            diagnostics::read_recent_logs,
            scraper::scrape_local_metadata,
            anilist::search_anilist,
            anilist::apply_anilist_metadata,
            bangumi::search_bangumi,
            bangumi::apply_bangumi_metadata,
            tmdb::tmdb_status,
            tmdb::save_tmdb_token,
            tmdb::search_tmdb,
            tmdb::apply_tmdb_metadata,
            scanner::scan_library,
            scanner::cancel_scan,
            scanner::scan_running
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
