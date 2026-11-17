mod commands;

use std::path::PathBuf;
use std::sync::Arc;

use slimlocalize_core::LocalizeService;
use slimlocalize_storage::SqliteStore;
use tauri::Manager;

#[derive(Debug)]
pub struct AppState {
    pub service: Arc<LocalizeService<SqliteStore>>,
    pub db_path: PathBuf,
    pub exports_dir: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = std::env::var_os("SLIMLOCALIZE_DB")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.join("slimlocalize.sqlite3"));
            let store = SqliteStore::open(&db_path)?;
            let service = Arc::new(LocalizeService::new(store));
            let exports_dir = data_dir.join("exports");
            std::fs::create_dir_all(&exports_dir)?;
            app.manage(AppState {
                service,
                db_path,
                exports_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::list_workspaces,
            commands::create_workspace,
            commands::list_projects,
            commands::create_project,
            commands::list_locales,
            commands::add_locale,
            commands::set_default_locale,
            commands::list_namespaces,
            commands::create_namespace,
            commands::list_translations,
            commands::save_translation,
            commands::list_environments,
            commands::create_environment,
            commands::list_config_variables,
            commands::set_config_variable,
            commands::check_project,
            commands::collab_bootstrap,
            commands::collab_export,
            commands::collab_import,
            commands::collab_rebuild_projection,
            commands::collab_status,
            commands::collab_validate,
            commands::export_project,
            commands::import_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SlimLocalize");
}
