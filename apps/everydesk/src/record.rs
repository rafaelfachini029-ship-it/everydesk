//! Session recording (controller side): where the engine's recorder writes and the
//! "Abrir pasta" button. The recorder itself runs in the engine's video thread
//! (`commands::session_record`); files are `outgoing_<id>_<ts>_display<N>_<codec>.webm`
//! (VP8/VP9/AV1) or `.mp4` (H264/H265, `hwcodec` builds only).

use std::path::Path;

/// Makes sure `dir` exists and returns it. Empty (the engine found no folder): an error.
fn ensure_dir(dir: &str) -> Result<String, String> {
    if dir.trim().is_empty() {
        return Err("Sem pasta para gravações".to_owned());
    }
    std::fs::create_dir_all(Path::new(dir))
        .map_err(|e| format!("Não foi possível criar a pasta de gravações: {e}"))?;
    Ok(dir.to_owned())
}

/// The folder the engine records into (`~/Movies/EveryDesk` on macOS, `Videos\EveryDesk` on
/// Windows, unless the `video-save-directory` option says otherwise), created if missing.
#[tauri::command]
pub fn recordings_dir() -> Result<String, String> {
    ensure_dir(&librustdesk::ui_interface::video_save_directory(false))
}

/// "Abrir pasta": opens the recordings folder in Finder / Explorer. Only this folder: the
/// path comes from the engine, never from the webview, so no opener capability is granted.
#[tauri::command]
pub fn open_recordings_dir() -> Result<(), String> {
    let dir = recordings_dir()?;
    tauri_plugin_opener::open_path(&dir, None::<&str>).map_err(|e| {
        log::warn!("everydesk: open {dir:?} failed: {e}");
        "Não foi possível abrir a pasta de gravações".to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_dir_creates_the_folder() {
        let base = std::env::temp_dir().join(format!("everydesk-rec-{}", std::process::id()));
        let dir = base.join("Movies").join("EveryDesk");
        let _ = std::fs::remove_dir_all(&base);
        let s = dir.to_string_lossy().to_string();
        assert_eq!(ensure_dir(&s).unwrap(), s);
        assert!(dir.is_dir());
        // Already there: fine.
        assert_eq!(ensure_dir(&s).unwrap(), s);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ensure_dir_refuses_empty() {
        assert!(ensure_dir("").is_err());
        assert!(ensure_dir("  ").is_err());
    }

}
