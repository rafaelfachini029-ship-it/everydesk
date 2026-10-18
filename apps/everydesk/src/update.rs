//! Auto-update (Etapa 5, Task 7). Checks GitHub Releases' `latest.json` 30 s after start and
//! then every 6 h; the UI offers "Nova versão x.y.z. Atualizar agora?" and nothing installs
//! without that click.
//!
//! Two routes, both gated by the minisign key in `tauri.conf.json` (`plugins.updater.pubkey`):
//! - **instalado** (this exe is the one in Program Files): `tauri-plugin-updater` downloads the
//!   NSIS setup (target `windows-x86_64-nsis` / `windows-x86_64`), verifies it and runs it with
//!   `/P /UPDATE /R` (passive, one UAC prompt). The setup's hooks stop and remove the service
//!   before copying (PREINSTALL) and reinstall + start it after (POSTINSTALL); `/R` reopens
//!   the app unelevated, without the `--hidden` it may have been started with.
//! - **portátil**: target `windows-x86_64-portable`; the plugin downloads and verifies the exe,
//!   `verify_package` checks it again against the same key (and the version it was signed
//!   for), then `self_replace` swaps the running exe and a detached relauncher starts it once
//!   this process is gone (single-instance `_show` would otherwise forward to us).
//!
//! Fail closed: no valid key (the placeholder shipped until Rafael generates one), not
//! Windows, or any check error → logged, nothing shown, nothing installed. A bad or missing
//! signature on a download → refused, the card says so, nothing replaced.

use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Extra `latest.json` platform for the portable exe (Task 8 writes it next to the NSIS one).
pub const PORTABLE_TARGET: &str = "windows-x86_64-portable";
/// Placeholder until `cargo tauri signer generate` (docs/release.md, "Chave do updater").
pub const PUBKEY_PLACEHOLDER: &str = "COLE_AQUI_A_CHAVE_PUBLICA_DO_UPDATER";
pub const STATUS_EVENT: &str = "update:status";
/// `update_install` error when someone is connected and the user has not confirmed yet.
pub const SESSION_ACTIVE: &str = "session-active";
const FIRST_CHECK: Duration = Duration::from_secs(30);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const SOURCE_URL: &str = "https://github.com/rafaelfachini029-ship-it/everydesk";
const LICENSE_URL: &str = "https://www.gnu.org/licenses/agpl-3.0.html";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// Not Windows, or no valid key: never checks.
    Disabled,
    /// This exe is the installed one: run the signed NSIS setup.
    Installer,
    /// Swap this exe for the signed portable one.
    Portable,
}

/// Pure: which route a build takes.
pub fn route(windows: bool, key_ok: bool, cur_exe_installed: bool) -> Route {
    match (windows, key_ok, cur_exe_installed) {
        (false, ..) | (_, false, _) => Route::Disabled,
        (true, true, true) => Route::Installer,
        (true, true, false) => Route::Portable,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Idle,
    Available,
    Downloading,
    Installing,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UpdateStatus {
    pub route: Route,
    pub phase: Phase,
    /// The announced version (available/downloading/installing/failed).
    pub version: Option<String>,
    /// 0-100 while downloading, when the server sent a length.
    pub progress: Option<u8>,
    /// pt-BR, only for `Failed`.
    pub message: Option<String>,
}

impl UpdateStatus {
    const fn idle(route: Route) -> Self {
        Self { route, phase: Phase::Idle, version: None, progress: None, message: None }
    }
}

static STATUS: Mutex<UpdateStatus> = Mutex::new(UpdateStatus::idle(Route::Disabled));
/// The update the card offers (kept so the click installs exactly what was announced).
static PENDING: Mutex<Option<Update>> = Mutex::new(None);

fn set_status(app: &AppHandle, f: impl FnOnce(&mut UpdateStatus)) {
    let st = {
        let mut s = STATUS.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    if let Err(e) = app.emit(STATUS_EVENT, st) {
        log::warn!("everydesk: emit {STATUS_EVENT}: {e}");
    }
}

/// A Tauri updater pubkey is base64 of a minisign public-key file. Anything else (empty, the
/// placeholder, a truncated paste) disables updates instead of failing later mid-install.
pub fn pubkey_usable(pubkey_b64: &str) -> bool {
    decode_b64_text(pubkey_b64)
        .ok()
        .and_then(|t| minisign_verify::PublicKey::decode(&t).ok())
        .is_some()
}

fn decode_b64_text(s: &str) -> Result<String, String> {
    use hbb_common::base64::Engine;
    let raw = hbb_common::base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .map_err(|e| e.to_string())?;
    String::from_utf8(raw).map_err(|e| e.to_string())
}

/// Parses a `latest.json` (the plugin's own parser) and returns the download URL and signature
/// for `target`. Used by the tests to pin the manifest shape Task 8 publishes.
#[cfg_attr(not(test), allow(dead_code))]
pub fn manifest_entry(json: &str, target: &str) -> Result<(String, String, String), String> {
    let rel: tauri_plugin_updater::RemoteRelease =
        serde_json::from_str(json).map_err(|e| e.to_string())?;
    let url = rel.download_url(target).map_err(|e| e.to_string())?.to_string();
    let sig = rel.signature(target).map_err(|e| e.to_string())?.clone();
    Ok((rel.version.to_string(), url, sig))
}

/// Text shown when a download does not match the key.
pub const BAD_SIGNATURE: &str =
    "A assinatura da atualização não confere. Nada foi instalado.";

/// The portable exe's own gate before `self_replace` (the plugin already checked the same bytes
/// in `download`; this one is ours, tested, and also insists on a PE file): the minisign
/// signature must verify with our key and its trusted comment must carry `version:<announced>`
/// (`tauri signer sign --app-version`), so an old signed exe cannot pose as a new one.
pub fn verify_package(
    bytes: &[u8],
    signature_b64: &str,
    pubkey_b64: &str,
    announced_version: &str,
) -> Result<(), String> {
    let pk = decode_b64_text(pubkey_b64)
        .and_then(|t| minisign_verify::PublicKey::decode(&t).map_err(|e| e.to_string()))
        .map_err(|e| format!("chave inválida: {e}"))?;
    let sig = decode_b64_text(signature_b64)
        .and_then(|t| minisign_verify::Signature::decode(&t).map_err(|e| e.to_string()))
        .map_err(|e| format!("assinatura ilegível: {e}"))?;
    pk.verify(bytes, &sig, true).map_err(|e| format!("assinatura inválida: {e}"))?;
    let signed = sig
        .trusted_comment()
        .split('\t')
        .find_map(|f| f.strip_prefix("version:"))
        .ok_or("assinatura sem versão")?;
    if signed.trim_start_matches('v') != announced_version.trim_start_matches('v') {
        return Err(format!("assinada para {signed}, anunciada como {announced_version}"));
    }
    if !bytes.starts_with(b"MZ") {
        return Err("não é um executável do Windows".into());
    }
    Ok(())
}

fn configured_pubkey(app: &AppHandle) -> String {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn cur_exe_installed() -> bool {
    #[cfg(windows)]
    return librustdesk::platform::is_cur_exe_the_installed();
    #[cfg(not(windows))]
    false
}

fn current_route(app: &AppHandle) -> Route {
    route(cfg!(windows), pubkey_usable(&configured_pubkey(app)), cur_exe_installed())
}

/// Someone is connected either way: our controller session, or a client of this host (`_cm`
/// sees the service's clients too).
fn session_active() -> bool {
    crate::commands::current().is_some() || librustdesk::ui_cm_interface::get_clients_length() > 0
}

/// Ends every session cleanly right before the swap/setup (the peers are told why instead of
/// seeing the host vanish): our controller session, and each client of this host, connected
/// or still asking (`ui_cm_interface::close` = the same `Data::Close` as "Encerrar").
fn close_sessions() {
    crate::commands::close_current();
    let snap = crate::cm::cm_pending();
    let ids = snap
        .connected
        .iter()
        .map(|c| c.conn_id)
        .chain(snap.pending.iter().map(|p| p.incoming.conn_id));
    for id in ids {
        log::info!("everydesk: update: closing #{id}");
        librustdesk::ui_cm_interface::close(id);
    }
    std::thread::sleep(Duration::from_millis(500));
}

pub fn start(app: AppHandle) {
    let r = current_route(&app);
    STATUS.lock().unwrap().route = r;
    if r == Route::Disabled {
        if !cfg!(windows) {
            log::info!("everydesk: updates disabled on this platform");
        } else if configured_pubkey(&app) == PUBKEY_PLACEHOLDER {
            log::warn!("everydesk: updates disabled: updater pubkey is still the placeholder");
        } else {
            log::warn!("everydesk: updates disabled: updater pubkey invalid");
        }
        return;
    }
    log::info!("everydesk: update route {r:?}");
    tauri::async_runtime::spawn(async move {
        hbb_common::tokio::time::sleep(FIRST_CHECK).await;
        loop {
            check(&app, r).await;
            hbb_common::tokio::time::sleep(EVERY).await;
        }
    });
}

fn updater(app: &AppHandle, r: Route) -> tauri_plugin_updater::Result<tauri_plugin_updater::Updater> {
    let mut b = app.updater_builder().timeout(CHECK_TIMEOUT);
    if r == Route::Portable {
        b = b.target(PORTABLE_TARGET);
    }
    // Runs after the package is verified and extracted, right before ShellExecute (UAC). No
    // `cleanup_before_exit` here: a declined prompt makes the plugin return an error and the
    // app must stay visible and usable ("Tentar de novo"); on success the plugin exits anyway.
    b.on_before_exit(close_sessions)
    // `/R` alone: reopen plainly (no `/ARGS`, so not `--hidden` after a click on "Atualizar").
    .installer_arg("/R")
    .build()
}

async fn check(app: &AppHandle, r: Route) {
    {
        let p = STATUS.lock().unwrap().phase;
        if matches!(p, Phase::Downloading | Phase::Installing) {
            return;
        }
    }
    let found = match updater(app, r) {
        Ok(u) => u.check().await,
        Err(e) => Err(e),
    };
    match found {
        Ok(Some(update)) => {
            let v = update.version.clone();
            log::info!("everydesk: update {v} available ({r:?})");
            *PENDING.lock().unwrap() = Some(update.restart_after_install(false));
            set_status(app, |s| {
                *s = UpdateStatus { phase: Phase::Available, version: Some(v), ..UpdateStatus::idle(r) }
            });
        }
        Ok(None) => log::info!("everydesk: no update"),
        // Offline, no release yet, no entry for this target...: quiet.
        Err(e) => log::warn!("everydesk: update check failed: {e}"),
    }
}

fn failure_text(e: &tauri_plugin_updater::Error) -> String {
    use tauri_plugin_updater::Error as E;
    match e {
        E::Minisign(_)
        | E::Base64(_)
        | E::SignatureUtf8(_)
        | E::SignedVersionMismatch { .. }
        | E::MissingSignedVersion => BAD_SIGNATURE.into(),
        E::Reqwest(_) | E::Network(_) => {
            "Não consegui baixar a atualização. Verifique a internet e tente de novo.".into()
        }
        _ => "Não consegui atualizar. Tente de novo mais tarde.".into(),
    }
}

#[tauri::command]
pub fn update_status() -> UpdateStatus {
    STATUS.lock().unwrap().clone()
}

/// "Atualizar agora". `force` = the user confirmed ending the active session.
#[tauri::command]
pub async fn update_install(app: AppHandle, force: bool) -> Result<(), String> {
    let Some(update) = PENDING.lock().unwrap().clone() else {
        return Err("Nenhuma atualização disponível.".into());
    };
    if !force && session_active() {
        return Err(SESSION_ACTIVE.into());
    }
    let r = {
        let mut s = STATUS.lock().unwrap();
        // one install at a time (a double click, or the card and a retry)
        if matches!(s.phase, Phase::Downloading | Phase::Installing) {
            return Err("A atualização já está em andamento.".into());
        }
        s.phase = Phase::Downloading;
        s.route
    };
    let v = update.version.clone();
    set_status(&app, |s| {
        s.progress = None;
        s.message = None;
    });
    let res = match r {
        Route::Installer => install_with_setup(&app, &update).await,
        Route::Portable => install_portable(&app, &update).await,
        Route::Disabled => Err(failure_text(&tauri_plugin_updater::Error::UnsupportedOs)),
    };
    if let Err(msg) = &res {
        log::error!("everydesk: update {v} not installed: {msg}");
        set_status(&app, |s| {
            s.phase = Phase::Failed;
            s.message = Some(msg.clone());
        });
    }
    res
}

fn progress_cb(app: &AppHandle) -> impl FnMut(usize, Option<u64>) {
    let app = app.clone();
    let mut got = 0u64;
    let mut last: Option<u8> = None;
    move |chunk, total| {
        got += chunk as u64;
        let pct = total.filter(|t| *t > 0).map(|t| ((got * 100) / t).min(100) as u8);
        // one event per percent, none at all without a Content-Length
        if pct.is_some() && pct != last {
            last = pct;
            set_status(&app, |s| s.progress = pct);
        }
    }
}

/// Windows: on success this never returns (the plugin runs the setup and exits the process).
/// Sessions stay up through the download and the signature check; `on_before_exit` ends them
/// just before the setup starts.
async fn install_with_setup(app: &AppHandle, update: &Update) -> Result<(), String> {
    let err = |e: tauri_plugin_updater::Error| {
        log::error!("everydesk: setup update: {e}");
        failure_text(&e)
    };
    // `download` verifies the signature (and the signed version) before returning the bytes.
    let bytes = update.download(progress_cb(app), || {}).await.map_err(err)?;
    set_status(app, |s| s.phase = Phase::Installing);
    // `install` verifies again, then runs the setup (UAC). A declined prompt lands here as Err.
    update.install(&bytes).map_err(err)
}

async fn install_portable(app: &AppHandle, update: &Update) -> Result<(), String> {
    let bytes = update.download(progress_cb(app), || {}).await.map_err(|e| {
        log::error!("everydesk: portable download: {e}");
        failure_text(&e)
    })?;
    verify_package(&bytes, &update.signature, &configured_pubkey(app), &update.version).map_err(|e| {
        log::error!("everydesk: portable package refused: {e}");
        BAD_SIGNATURE.to_string()
    })?;
    // Stage next to the exe before touching any session: a read-only folder fails here.
    let exe = std::env::current_exe().map_err(|e| {
        log::error!("everydesk: current_exe: {e}");
        SWAP_FAILED.to_string()
    })?;
    let dir = exe.parent().ok_or(SWAP_FAILED)?;
    let staged = Staged::write(dir, &bytes)?;
    set_status(app, |s| s.phase = Phase::Installing);
    close_sessions();
    swap_and_relaunch(app, &exe, &staged, &bytes)
}

const SWAP_FAILED: &str = "Não consegui trocar o executável. Feche o EveryDesk e baixe a nova versão.";
pub const READ_ONLY_DIR: &str = "A pasta do EveryDesk não permite gravar, então não dá para atualizar aqui. \
Mova o EveryDesk.exe para uma pasta sua (por exemplo, Downloads) ou baixe a nova versão.";

/// The verified exe, written next to the running one under a fresh name (`create_new`: never a
/// file someone placed there before), removed on every path when dropped.
pub struct Staged {
    pub path: std::path::PathBuf,
}

impl Staged {
    pub fn write(dir: &std::path::Path, bytes: &[u8]) -> Result<Self, String> {
        use std::io::Write;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let path = dir.join(format!(".EveryDesk-update-{}-{nanos}.exe", std::process::id()));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| {
                log::error!("everydesk: stage {}: {e}", path.display());
                READ_ONLY_DIR.to_string()
            })?;
        let staged = Staged { path };
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| {
                log::error!("everydesk: stage write: {e}");
                READ_ONLY_DIR.to_string()
            })?;
        Ok(staged)
    }

    /// The file still holds exactly the verified bytes (checked right before `self_replace`
    /// reads it again).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn matches(&self, verified: &[u8]) -> bool {
        std::fs::read(&self.path).map(|b| b == verified).unwrap_or(false)
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(windows)]
fn swap_and_relaunch(
    app: &AppHandle,
    exe: &std::path::Path,
    staged: &Staged,
    verified: &[u8],
) -> Result<(), String> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let fail = |e: String| {
        log::error!("everydesk: portable swap: {e}");
        SWAP_FAILED.to_string()
    };
    if !staged.matches(verified) {
        return Err(fail("staged file changed after verification".into()));
    }
    self_replace::self_replace(&staged.path).map_err(|e| fail(e.to_string()))?;
    let exe = exe.to_string_lossy().into_owned();
    // Waits for this pid to exit, then starts the (new) exe; Task 3's script.
    let script = crate::service::relaunch_script(
        crate::APP_NAME,
        &exe,
        std::process::id(),
        crate::service::WaitFor::Nothing,
    )
    .ok_or_else(|| fail("caminho inválido".into()))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir()
        .join(format!("EveryDesk-update-relaunch-{}-{nanos}.cmd", std::process::id()));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut f| f.write_all(script.as_bytes()))
        .map_err(|e| fail(e.to_string()))?;
    std::process::Command::new("cmd")
        .arg("/c")
        .arg(&path)
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map_err(|e| fail(e.to_string()))?;
    log::info!("everydesk: portable exe replaced; relaunching");
    app.exit(0);
    Ok(())
}

#[cfg(not(windows))]
fn swap_and_relaunch(
    _app: &AppHandle,
    _exe: &std::path::Path,
    _staged: &Staged,
    _verified: &[u8],
) -> Result<(), String> {
    Err("Atualização automática disponível no Windows nesta versão.".into())
}

/// "Depois": hides the card until the next check finds something (the UI keeps it hidden for
/// this version); also clears a failure.
#[tauri::command]
pub fn update_dismiss(app: AppHandle) {
    let p = STATUS.lock().unwrap().phase;
    if p == Phase::Failed {
        set_status(&app, |s| {
            s.phase = Phase::Available;
            s.message = None;
            s.progress = None;
        });
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct About {
    pub version: String,
    /// Short commit of the build (`GITHUB_SHA` in CI, or `EVERYDESK_COMMIT`), `None` locally.
    pub commit: Option<String>,
    pub source_url: String,
    pub license: String,
}

pub fn build_commit() -> Option<String> {
    option_env!("EVERYDESK_COMMIT")
        .or(option_env!("GITHUB_SHA"))
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(12).collect())
}

/// Source of exactly this build when the commit is known (AGPL-3.0 §6), else the repository.
pub fn source_url(commit: Option<&str>) -> String {
    match commit {
        Some(c) => format!("{SOURCE_URL}/tree/{c}"),
        None => SOURCE_URL.to_owned(),
    }
}

#[tauri::command]
pub fn about_info(app: AppHandle) -> About {
    let commit = build_commit();
    About {
        version: app.package_info().version.to_string(),
        source_url: source_url(commit.as_deref()),
        commit,
        license: "AGPL-3.0".into(),
    }
}

/// Opens a fixed URL (source code or license) in the browser; the webview never picks the URL.
#[tauri::command]
pub fn about_open(which: String) -> Result<(), String> {
    let url = match which.as_str() {
        "source" => source_url(build_commit().as_deref()),
        "license" => LICENSE_URL.to_owned(),
        _ => return Err("link desconhecido".into()),
    };
    tauri_plugin_opener::open_url(&url, None::<&str>).map_err(|e| {
        log::warn!("everydesk: open {url}: {e}");
        "Não consegui abrir o navegador.".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::base64::Engine;

    fn b64(s: &str) -> String {
        hbb_common::base64::engine::general_purpose::STANDARD.encode(s)
    }

    /// A key pair and a Tauri-style signature (`tauri signer sign --app-version`) over `data`.
    fn signed(data: &[u8], version: &str) -> (String, String) {
        let kp = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let pk = b64(&kp.pk.to_box().unwrap().into_string());
        let comment = format!("timestamp:1700000000\tfile:EveryDesk.exe\tversion:{version}");
        let sig = minisign::sign(Some(&kp.pk), &kp.sk, std::io::Cursor::new(data), Some(&comment), None)
            .unwrap();
        (pk, b64(&sig.into_string()))
    }

    const MANIFEST: &str = r#"{
      "version": "0.2.0",
      "notes": "EveryDesk 0.2.0",
      "pub_date": "2026-10-20T12:00:00Z",
      "platforms": {
        "windows-x86_64": {
          "url": "https://github.com/rafaelfachini029-ship-it/everydesk/releases/download/v0.2.0/EveryDesk_0.2.0_x64-setup.exe",
          "signature": "c2V0dXA="
        },
        "windows-x86_64-portable": {
          "url": "https://github.com/rafaelfachini029-ship-it/everydesk/releases/download/v0.2.0/EveryDesk.exe",
          "signature": "cG9ydGFibGU="
        }
      }
    }"#;

    #[test]
    fn manifest_has_setup_and_portable_targets() {
        let (v, url, sig) = manifest_entry(MANIFEST, PORTABLE_TARGET).unwrap();
        assert_eq!(v, "0.2.0");
        assert!(url.ends_with("/v0.2.0/EveryDesk.exe"), "{url}");
        assert_eq!(sig, "cG9ydGFibGU=");
        let (_, url, sig) = manifest_entry(MANIFEST, "windows-x86_64").unwrap();
        assert!(url.ends_with("_x64-setup.exe"), "{url}");
        assert_eq!(sig, "c2V0dXA=");
        assert!(manifest_entry(MANIFEST, "darwin-aarch64").is_err());
        // a `v` prefix is accepted, a missing version is not
        assert!(manifest_entry(&MANIFEST.replace("\"0.2.0\"", "\"v0.2.0\""), PORTABLE_TARGET).is_ok());
        assert!(manifest_entry(r#"{"platforms":{}}"#, PORTABLE_TARGET).is_err());
    }

    #[test]
    fn placeholder_or_garbage_key_disables_updates() {
        assert!(!pubkey_usable(PUBKEY_PLACEHOLDER));
        assert!(!pubkey_usable(""));
        assert!(!pubkey_usable(&b64("untrusted comment: x\nnot a key\n")));
        let (pk, _) = signed(b"MZ", "0.2.0");
        assert!(pubkey_usable(&pk));
        assert_eq!(route(true, false, true), Route::Disabled);
        assert_eq!(route(false, true, false), Route::Disabled);
        assert_eq!(route(true, true, true), Route::Installer);
        assert_eq!(route(true, true, false), Route::Portable);
    }

    #[test]
    fn good_signature_is_accepted() {
        let exe = b"MZ\x90\x00 portable exe";
        let (pk, sig) = signed(exe, "0.2.0");
        assert_eq!(verify_package(exe, &sig, &pk, "0.2.0"), Ok(()));
        assert_eq!(verify_package(exe, &sig, &pk, "v0.2.0"), Ok(()));
    }

    #[test]
    fn bad_signature_is_refused() {
        let exe = b"MZ\x90\x00 portable exe";
        let (pk, sig) = signed(exe, "0.2.0");
        // tampered bytes
        let mut evil = exe.to_vec();
        evil.push(0);
        assert!(verify_package(&evil, &sig, &pk, "0.2.0").is_err());
        // signed by another key
        let (other_pk, _) = signed(exe, "0.2.0");
        assert!(verify_package(exe, &sig, &other_pk, "0.2.0").is_err());
        // missing / garbage signature
        assert!(verify_package(exe, "", &pk, "0.2.0").is_err());
        assert!(verify_package(exe, &b64("garbage"), &pk, "0.2.0").is_err());
        // placeholder key: never accepts anything
        assert!(verify_package(exe, &sig, PUBKEY_PLACEHOLDER, "0.2.0").is_err());
        // an older signed exe announced as a newer version
        assert!(verify_package(exe, &sig, &pk, "0.3.0").is_err());
        // validly signed but not a PE file
        let (pk2, sig2) = signed(b"#!/bin/sh", "0.2.0");
        assert!(verify_package(b"#!/bin/sh", &sig2, &pk2, "0.2.0").is_err());
    }

    #[test]
    fn signature_without_version_is_refused() {
        let kp = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let pk = b64(&kp.pk.to_box().unwrap().into_string());
        let exe = b"MZ old";
        let sig = minisign::sign(Some(&kp.pk), &kp.sk, std::io::Cursor::new(&exe[..]), Some("timestamp:1"), None)
            .unwrap();
        let err = verify_package(exe, &b64(&sig.into_string()), &pk, "0.2.0").unwrap_err();
        assert!(err.contains("sem versão"), "{err}");
    }

    /// The shipped key is either the marked placeholder (updates off) or a real, decodable
    /// key: a truncated paste must fail here, not silently on users' machines.
    #[test]
    fn configured_key_is_placeholder_or_valid() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = &conf["plugins"]["updater"];
        let pk = updater["pubkey"].as_str().unwrap();
        assert!(pk == PUBKEY_PLACEHOLDER || pubkey_usable(pk), "invalid updater pubkey");
        assert_eq!(updater["windows"]["installMode"], "passive");
        assert_eq!(updater["requireSignedVersion"], true);
        assert_eq!(
            updater["endpoints"][0],
            "https://github.com/rafaelfachini029-ship-it/everydesk/releases/latest/download/latest.json"
        );
    }

    #[test]
    fn staged_file_is_fresh_checked_and_removed() {
        let dir = std::env::temp_dir().join(format!("everydesk-stage-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bytes = b"MZ verified";
        let staged = Staged::write(&dir, bytes).unwrap();
        let path = staged.path.clone();
        assert!(path.starts_with(&dir));
        assert!(staged.matches(bytes));
        // swapped under our feet after verification -> refused
        std::fs::write(&path, b"MZ evil").unwrap();
        assert!(!staged.matches(bytes));
        drop(staged);
        assert!(!path.exists(), "staged file left behind");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn read_only_folder_fails_before_anything_else() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("everydesk-ro-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let res = Staged::write(&dir, b"MZ");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        // (root can write anywhere; the check is meaningful for a normal user)
        if let Err(e) = res {
            assert_eq!(e, READ_ONLY_DIR);
        }
    }

    #[test]
    fn status_json_shape() {
        let s = UpdateStatus {
            phase: Phase::Available,
            version: Some("0.2.0".into()),
            ..UpdateStatus::idle(Route::Portable)
        };
        let j = serde_json::to_value(&s).unwrap();
        assert_eq!(j["route"], "portable");
        assert_eq!(j["phase"], "available");
        assert_eq!(j["version"], "0.2.0");
        assert!(j["progress"].is_null());
    }

    #[test]
    fn about_links_point_at_this_build() {
        assert_eq!(
            source_url(Some("82b42c3ea")),
            "https://github.com/rafaelfachini029-ship-it/everydesk/tree/82b42c3ea"
        );
        assert_eq!(source_url(None), SOURCE_URL);
    }
}
