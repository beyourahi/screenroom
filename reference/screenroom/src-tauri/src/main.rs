#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde::Serialize;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use walkdir::WalkDir;

#[derive(Serialize)]
struct MediaFile { name: String, path: String, size: u64, source: String }

// The UI cannot supply an arbitrary path: access begins with an OS folder picker.
// Symlinks are not followed and all filesystem operations are read-only.
#[tauri::command]
async fn pick_and_scan(app: tauri::AppHandle) -> Result<Vec<MediaFile>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(selection) = app.dialog().file().set_title("Connect a media folder or external drive").blocking_pick_folder() else { return Ok(vec![]) };
        let root = selection.into_path().map_err(|e| e.to_string())?.canonicalize().map_err(|e| e.to_string())?;
        app.asset_protocol_scope().allow_directory(&root, true).map_err(|e| e.to_string())?;
        let source = root.to_string_lossy().into_owned();
        let mut items = Vec::new();
        for entry in WalkDir::new(&root).follow_links(false) {
            let entry = entry.map_err(|e| format!("Could not read a source entry: {e}"))?;
            if !entry.file_type().is_file() { continue; }
            let extension = entry.path().extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
            if !["mp4","mkv","m4v","mov","webm","avi","ts","m2ts","mpg","mpeg"].contains(&extension.as_str()) { continue; }
            let path = entry.path().canonicalize().map_err(|e| e.to_string())?;
            if !path.starts_with(&root) { continue; }
            let metadata = path.metadata().map_err(|e| e.to_string())?;
            items.push(MediaFile {name:entry.file_name().to_string_lossy().into_owned(),path:path.to_string_lossy().into_owned(),size:metadata.len(),source:source.clone()});
        }
        Ok(items)
    }).await.map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![pick_and_scan])
        .run(tauri::generate_context!())
        .expect("Screenroom could not start");
}
