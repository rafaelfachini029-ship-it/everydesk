//! What the engine installer (`--silent-install` / `--update`) copies into the install dir.
//!
//! Stock RustDesk XCOPYs the whole folder of the running exe. For EveryDesk that folder is
//! wherever the user saved the portable exe (Downloads, Desktop, ...): user-writable, so a
//! same-user process could drop e.g. `version.dll` there and have it copied into
//! `C:\Program Files\EveryDesk`, next to the exe the SYSTEM service runs. EveryDesk is a single
//! self-contained exe (Tauri 2 links the WebView2 loader statically on MSVC; `LICENCE.txt` from
//! `bundle.resources` is documentation, never read at runtime), so only that exe is copied.

/// Files copied into the install dir, as `(source, destination)`: the running exe only,
/// written under its canonical name (the download may be called `EveryDesk (1).exe`).
pub fn files_to_install(src_exe: &str, dest_exe: &str) -> Vec<(String, String)> {
    vec![(src_exe.to_owned(), dest_exe.to_owned())]
}

/// `cmd` lines copying [`files_to_install`]. `install_me` rejects paths with quotes or `%`
/// (`validate_install_value`) before they reach a script.
pub fn copy_files_cmd(src_exe: &str, dest_exe: &str) -> String {
    files_to_install(src_exe, dest_exe)
        .iter()
        .map(|(src, dst)| format!("copy /Y \"{src}\" \"{dst}\""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everydesk_install_copies_only_the_exe() {
        let files = files_to_install(
            r"C:\Users\u\Downloads\EveryDesk (1).exe",
            r"C:\Program Files\EveryDesk\EveryDesk.exe",
        );
        assert_eq!(
            files,
            vec![(
                r"C:\Users\u\Downloads\EveryDesk (1).exe".to_owned(),
                r"C:\Program Files\EveryDesk\EveryDesk.exe".to_owned()
            )]
        );
    }

    #[test]
    fn everydesk_install_cmd_never_copies_the_folder() {
        let cmd = copy_files_cmd(
            r"C:\Users\u\Downloads\EveryDesk.exe",
            r"C:\Program Files\EveryDesk\EveryDesk.exe",
        );
        assert_eq!(
            cmd,
            r#"copy /Y "C:\Users\u\Downloads\EveryDesk.exe" "C:\Program Files\EveryDesk\EveryDesk.exe""#
        );
        let lower = cmd.to_lowercase();
        assert!(!lower.contains("xcopy") && !lower.contains("robocopy"));
        assert!(!lower.contains("downloads\"") && !lower.contains("downloads\\\""));
    }
}
