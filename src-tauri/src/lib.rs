use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tokio::task::JoinHandle;

pub mod commands;
pub mod db;
pub mod grouper;
pub mod processor;
pub mod scanner;
pub mod settings;

use settings::AppSettings;

pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub scan_handle: Mutex<Option<JoinHandle<()>>>,
    pub job_handles: Mutex<HashMap<i64, JoinHandle<()>>>,
    pub settings: RwLock<AppSettings>,
}

impl AppState {
    pub fn new(db: sqlx::SqlitePool, settings: AppSettings) -> Arc<Self> {
        Arc::new(Self {
            db,
            scan_handle: Mutex::new(None),
            job_handles: Mutex::new(HashMap::new()),
            settings: RwLock::new(settings),
        })
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            let state = tauri::async_runtime::block_on(async move {
                let db_path = tauri::Manager::path(&app_handle)
                    .app_data_dir()
                    .expect("failed to get app data dir");
                std::fs::create_dir_all(&db_path).expect("failed to create app data dir");

                let db_url = format!(
                    "sqlite://{}",
                    db_path.join("gallery.db").to_str().unwrap()
                );

                let pool = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect_with(
                        db_url
                            .parse::<sqlx::sqlite::SqliteConnectOptions>()
                            .unwrap()
                            .create_if_missing(true)
                            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
                    )
                    .await
                    .expect("failed to connect to database");

                sqlx::migrate!("src/db/migrations")
                    .run(&pool)
                    .await
                    .expect("failed to run migrations");

                // Create thumbnail cache dir
                let thumb_dir = db_path.join("thumbnails");
                std::fs::create_dir_all(&thumb_dir).expect("failed to create thumbnail dir");

                let settings = settings::load_all(&pool)
                    .await
                    .expect("failed to load settings");

                AppState::new(pool, settings)
            });

            tauri::Manager::manage(app, state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan::scan_folder,
            commands::scan::get_scan_status,
            commands::scan::cancel_scan,
            commands::groups::get_groups,
            commands::groups::rebuild_groups,
            commands::groups::get_duplicate_clusters,
            commands::groups::set_keeper,
            commands::groups::dismiss_cluster,
            commands::groups::remove_group,
            commands::groups::remove_image_from_group,
            commands::jobs::create_job,
            commands::jobs::start_job,
            commands::jobs::cancel_job,
            commands::jobs::get_jobs,
            commands::jobs::get_job_detail,
            commands::jobs::retry_failed_images,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::open_folder_dialog,
            commands::settings::reveal_in_explorer,
            commands::images::get_images,
            commands::images::get_image_detail,
            commands::images::get_thumbnail,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
