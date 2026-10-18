//! Permanent access (spec 4.6, objective 8): the app either hosts in its own process ("portátil",
//! works while the window is open) or, once installed as the Windows service, only syncs with the
//! service's `--server` over IPC ("instalado": starts with Windows, reachable on the lock screen).
//!
//! Who hosts is decided once at startup (`is_service_mode`) and then re-checked by a watcher:
//! - the service appears while this process hosts in-process -> hand over: relaunch the installed
//!   exe after this process exits and exit cleanly, so the service's `--server` can take the
//!   `""` IPC pipe (otherwise it would `Close`/terminate us, `server.rs` `stop_main_window_process`);
//! - the service disappears while we only synced with it -> host in-process again
//!   (`start_server(false, false)`), so the machine stays reachable while the app is open, and
//!   tell the UI "serviço parado".
//!
//! Every elevation goes through one UAC prompt (`run_uac`); nothing here needs admin otherwise.

use serde::Serialize;
#[cfg(windows)]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Service name = `crate::get_app_name()` in the engine (APP_NAME, see `lib.rs` branding).
#[cfg_attr(not(windows), allow(dead_code))]
const SERVICE_NAME: &str = crate::APP_NAME;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Hosts in this process: works while the app is open.
    Portable,
    /// The Windows service hosts (`--server`); this process only syncs with it.
    Installed,
    /// Installed and expected to run, but the service is not running.
    InstalledStopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ServiceStatus {
    pub mode: Mode,
    pub platform_supported: bool,
}

/// Pure decision. `stop_requested` is the engine's `stop-service` option, which
/// `--uninstall-service` sets: an installed copy whose service was removed on purpose is just
/// portable again, not "parado". A running service with `stop-service=Y` does not spawn
/// `--server`, so it does not host either.
pub fn service_mode_decision(
    installed: bool,
    running: bool,
    stop_requested: bool,
    platform: Platform,
) -> Mode {
    match (platform, installed, running, stop_requested) {
        (Platform::Other, ..) => Mode::Portable,
        (_, false, ..) => Mode::Portable,
        (_, true, _, true) => Mode::Portable,
        (_, true, true, false) => Mode::Installed,
        (_, true, false, false) => Mode::InstalledStopped,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Nothing,
    /// The service hosts now and so does this process: exit and relaunch the installed exe.
    HandOver,
    /// Nobody hosts: start the in-process host (`start_server(false, false)`).
    HostInProcess,
}

/// Pure watcher step. `hosting` = this process runs the in-process host
/// (`common::is_server_running`); `missing_polls` = consecutive polls without the service while
/// this process did not host (a `--server` restart by the service is not a disappearance).
pub fn watcher_action(mode: Mode, hosting: bool, missing_polls: u32) -> Action {
    match mode {
        Mode::Installed if hosting => Action::HandOver,
        Mode::Installed => Action::Nothing,
        _ if !hosting && missing_polls >= 2 => Action::HostInProcess,
        _ => Action::Nothing,
    }
}

fn current_platform() -> Platform {
    if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Other
    }
}

pub fn status() -> ServiceStatus {
    #[cfg(windows)]
    let (installed, running) = (
        librustdesk::platform::is_installed(),
        librustdesk::platform::is_self_service_running(),
    );
    #[cfg(not(windows))]
    let (installed, running) = (false, false);
    let stop = hbb_common::config::Config::get_option("stop-service") == "Y";
    ServiceStatus {
        mode: service_mode_decision(installed, running, stop, current_platform()),
        platform_supported: current_platform() == Platform::Windows,
    }
}

/// The service hosts: this process must never host in-process (`start_server(false, true)`).
pub fn is_service_mode() -> bool {
    status().mode == Mode::Installed
}

/// This process runs the in-process host (and so owns the `""` IPC pipe).
pub fn hosting_in_process() -> bool {
    librustdesk::common::is_server_running()
}

/// `is_service_mode` as of the watcher's last poll (every 1-5 s, `start_watcher`), so callers on
/// hot paths do not query the service manager; a direct query only before the first poll.
pub fn cached_service_mode() -> bool {
    if let Some(st) = *LAST.lock().unwrap() {
        return st.mode == Mode::Installed;
    }
    is_service_mode()
}

static LAST: Mutex<Option<ServiceStatus>> = Mutex::new(None);
/// Set after a UAC prompt was accepted: poll fast until the installer is done, so we hand over
/// before the service's `--server` kills this process.
#[cfg(windows)]
static FAST_UNTIL: AtomicU64 = AtomicU64::new(0);
static FALLBACK_HOST: AtomicBool = AtomicBool::new(false);

#[cfg_attr(not(windows), allow(dead_code))]
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn poll_interval(hosting: bool) -> Duration {
    #[cfg(windows)]
    {
        if FAST_UNTIL.load(Ordering::Relaxed) > now_secs() {
            return Duration::from_millis(400);
        }
        if hosting {
            // A service that comes up behind our back (NSIS, `sc start`) kills an in-process
            // host within a few seconds of starting: notice it first.
            return Duration::from_secs(1);
        }
    }
    let _ = hosting;
    Duration::from_secs(5)
}

/// Starts the 5 s watcher (faster while it matters, see `poll_interval`); emits
/// `service:status` on every change.
pub fn start_watcher(app: AppHandle) {
    #[cfg(windows)]
    let _ = librustdesk::ipc::ON_EXIT_RECV_CLOSE.set(relaunch_on_close);
    std::thread::Builder::new()
        .name("everydesk-service".into())
        .spawn(move || {
            let mut missing = 0u32;
            loop {
                let st = status();
                let hosting = hosting_in_process();
                let changed = {
                    let mut last = LAST.lock().unwrap();
                    let changed = *last != Some(st);
                    *last = Some(st);
                    changed
                };
                if changed {
                    log::info!("everydesk: service status {st:?} (in-process host: {hosting})");
                    if let Err(e) = app.emit("service:status", st) {
                        log::warn!("everydesk: emit service:status: {e}");
                    }
                }
                missing = if st.mode == Mode::Installed || hosting { 0 } else { missing + 1 };
                match watcher_action(st.mode, hosting, missing) {
                    Action::HandOver => hand_over(&app),
                    Action::HostInProcess => {
                        if !FALLBACK_HOST.swap(true, Ordering::SeqCst) {
                            log::info!("everydesk: service gone, hosting in-process");
                            std::thread::spawn(|| librustdesk::start_server(false, false));
                        }
                    }
                    Action::Nothing => {}
                }
                std::thread::sleep(poll_interval(hosting));
            }
        })
        .expect("spawn service watcher");
}

/// The engine has no way to stop an in-process host, so the clean stop is to exit: relaunch
/// the installed exe once we are gone (it starts in service mode and only syncs) and quit.
#[cfg_attr(not(windows), allow(unused_variables))]
fn hand_over(app: &AppHandle) {
    #[cfg(windows)]
    {
        static ONCE: AtomicBool = AtomicBool::new(false);
        if ONCE.swap(true, Ordering::SeqCst) {
            return;
        }
        let (_, _, _, exe) = librustdesk::platform::get_install_info();
        log::info!("everydesk: service is hosting now; handing over to {exe}");
        if !RELAUNCH_PENDING.swap(true, Ordering::SeqCst) {
            if let Err(e) = win::spawn_relauncher(&exe, WaitFor::Nothing) {
                log::error!("everydesk: relauncher: {e}");
            }
        }
        app.exit(0);
    }
}

/// A relauncher is already waiting for this process to go (install/remove/hand-over): a
/// second one would only start a duplicate window.
#[cfg_attr(not(windows), allow(dead_code))]
static RELAUNCH_PENDING: AtomicBool = AtomicBool::new(false);

/// `ipc::ON_EXIT_RECV_CLOSE`: a service started behind our back (NSIS while the portable app is
/// open, `sc start`) has its `--server` send `Close` to this in-process host before the watcher
/// noticed it; the engine then exits this process. Come back once the service runs.
#[cfg(windows)]
fn relaunch_on_close() {
    if RELAUNCH_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let exe = relaunch_exe();
    log::info!("everydesk: closed by the service's host; relaunching {exe} once it runs");
    if let Err(e) = win::spawn_relauncher(&exe, WaitFor::ServiceRunning) {
        log::error!("everydesk: relauncher: {e}");
    }
}

/// The installed exe when there is one, else this one.
#[cfg(windows)]
fn relaunch_exe() -> String {
    if librustdesk::platform::is_installed() {
        librustdesk::platform::get_install_info().3
    } else {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| librustdesk::platform::get_install_info().3)
    }
}

/// What the detached relauncher waits for before starting the app again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum WaitFor {
    Nothing,
    ServiceRunning,
    ServiceStopped,
}

/// A `cmd` script (not EveryDesk.exe, so the installers' `taskkill /IM EveryDesk.exe` spares it)
/// that waits up to ~3 min until process `pid` (the app that spawned it) is gone *and* `wait`
/// holds, then starts `exe` a few seconds later; on timeout it launches nothing. Waiting for
/// `pid` keeps it from starting a second window while the UAC prompt is still open. `None`
/// when the path cannot be quoted safely.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn relaunch_script(service: &str, exe: &str, pid: u32, wait: WaitFor) -> Option<String> {
    let bad = |s: &str| s.is_empty() || s.contains(['"', '\r', '\n']);
    if bad(service) || bad(exe) {
        return None;
    }
    // `%` is the only character a quoted batch argument still expands.
    let exe = exe.replace('%', "%%");
    let service = service.replace('%', "%%");
    let ready = match wait {
        WaitFor::Nothing => "goto launch".to_string(),
        WaitFor::ServiceRunning => format!(
            "sc query \"{service}\" | find \"RUNNING\" >nul\r\n    if not errorlevel 1 if exist \"{exe}\" goto launch"
        ),
        WaitFor::ServiceStopped => format!(
            "sc query \"{service}\" | find \"RUNNING\" >nul\r\n    if errorlevel 1 goto launch"
        ),
    };
    Some(format!(
        "@echo off\r\nfor /l %%i in (1,1,90) do (\r\n  tasklist /FI \"PID eq {pid}\" /NH | find \" {pid} \" >nul\r\n  if errorlevel 1 (\r\n    {ready}\r\n  )\r\n  ping -n 3 127.0.0.1 >nul\r\n)\r\ndel \"%~f0\" & exit /b 0\r\n:launch\r\nping -n 4 127.0.0.1 >nul\r\nstart \"\" \"{exe}\"\r\ndel \"%~f0\"\r\n"
    ))
}

#[cfg(windows)]
mod win {
    use super::{relaunch_script, WaitFor, SERVICE_NAME};
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    /// Writes a fresh script (unique name: cmd reads a batch file while running it) and
    /// starts it detached. The child is returned so a declined UAC prompt can cancel it.
    pub fn spawn_relauncher(exe: &str, wait: WaitFor) -> Result<std::process::Child, String> {
        let script = relaunch_script(SERVICE_NAME, exe, std::process::id(), wait)
            .ok_or("caminho inválido")?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "EveryDesk-relaunch-{}-{nanos}.cmd",
            std::process::id()
        ));
        std::fs::write(&path, script).map_err(|e| e.to_string())?;
        std::process::Command::new("cmd")
            .arg("/c")
            .arg(&path)
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()
            .map_err(|e| e.to_string())
    }

    /// Relauncher first, UAC second: once the prompt is accepted the elevated
    /// `--install-service`/`--silent-install`/`--uninstall-service` runs `taskkill /IM
    /// EveryDesk.exe`, which can hit this process before `run_uac` even returns. A declined
    /// prompt kills the relauncher again (it would only time out, but never launch twice).
    pub fn uac_with_relauncher(
        uac_exe: &str,
        arg: &str,
        relaunch_exe: &str,
        wait: WaitFor,
    ) -> Result<(), String> {
        let mut child = spawn_relauncher(relaunch_exe, wait)?;
        super::RELAUNCH_PENDING.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Err(e) = uac(uac_exe, arg) {
            let _ = child.kill();
            let _ = child.wait();
            super::RELAUNCH_PENDING.store(false, std::sync::atomic::Ordering::SeqCst);
            return Err(e);
        }
        Ok(())
    }

    pub fn uac(exe: &str, arg: &str) -> Result<(), String> {
        match librustdesk::platform::run_uac(exe, arg) {
            Ok(true) => Ok(()),
            Ok(false) => Err("A permissão de administrador não foi concedida.".into()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(windows)]
fn fast_poll_for(secs: u64) {
    FAST_UNTIL.store(now_secs() + secs, Ordering::Relaxed);
}

#[cfg_attr(windows, allow(dead_code))]
const UNSUPPORTED: &str = "Disponível no Windows nesta versão.";

/// "Instalar para acesso permanente" (and "Reiniciar serviço"). One UAC prompt.
/// - Already installed (NSIS or an earlier install): `--install-service` recreates/starts the
///   service.
/// - Portable: this very exe is elevated with `--silent-install`, which copies only the exe
///   (never its folder, see `librustdesk::everydesk::install`) to Program Files and creates the
///   service. No staging copy in `%TEMP%`: a user-writable folder the elevated process runs
///   from is where a planted DLL would get loaded or installed.
/// Both kill every other EveryDesk.exe (this window too) and, under the `everydesk` feature,
/// never relaunch (`run_after_run_cmds`), so a detached unelevated relauncher, started before
/// the prompt, starts the installed exe once the service runs.
#[tauri::command]
pub fn install_permanent() -> Result<(), String> {
    #[cfg(windows)]
    {
        let (_, _, _, installed_exe) = librustdesk::platform::get_install_info();
        let wait = WaitFor::ServiceRunning;
        if librustdesk::platform::is_installed() {
            win::uac_with_relauncher(&installed_exe, "--install-service", &installed_exe, wait)?;
        } else {
            let cur = std::env::current_exe().map_err(|e| e.to_string())?;
            let cur = cur.to_string_lossy();
            win::uac_with_relauncher(&cur, "--silent-install", &installed_exe, wait)?;
        }
        fast_poll_for(180);
        return Ok(());
    }
    #[cfg(not(windows))]
    Err(UNSUPPORTED.into())
}

/// "Remover acesso permanente": removes the service and its login autostart (the HKLM Run
/// value, `--uninstall-service` drops it like the NSIS uninstaller), keeps the installed
/// program (portable mode from then on). `--uninstall-service` kills every EveryDesk.exe and does not relaunch,
/// so the relauncher brings this app back once the service stopped.
#[tauri::command]
pub fn uninstall_permanent() -> Result<(), String> {
    #[cfg(windows)]
    {
        let (_, _, _, installed_exe) = librustdesk::platform::get_install_info();
        let exe = if librustdesk::platform::is_installed() {
            installed_exe
        } else {
            std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .into_owned()
        };
        win::uac_with_relauncher(&exe, "--uninstall-service", &exe, WaitFor::ServiceStopped)?;
        return Ok(());
    }
    #[cfg(not(windows))]
    Err(UNSUPPORTED.into())
}

#[tauri::command]
pub fn service_status() -> ServiceStatus {
    status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_decision_table() {
        use Mode::*;
        use Platform::*;
        let rows = [
            // installed, running, stop-service, platform => mode
            (false, false, false, Windows, Portable),
            (false, true, false, Windows, Portable),
            (true, true, false, Windows, Installed),
            (true, false, false, Windows, InstalledStopped),
            (true, false, true, Windows, Portable),
            (true, true, true, Windows, Portable),
            (true, true, false, Other, Portable),
            (false, false, false, Other, Portable),
        ];
        for (i, r, s, p, want) in rows {
            assert_eq!(service_mode_decision(i, r, s, p), want, "{i} {r} {s} {p:?}");
        }
    }

    #[test]
    fn watcher_hands_over_and_falls_back() {
        use Action::*;
        assert_eq!(watcher_action(Mode::Installed, true, 0), HandOver);
        assert_eq!(watcher_action(Mode::Installed, false, 0), Nothing);
        assert_eq!(watcher_action(Mode::Portable, true, 0), Nothing);
        // One missed poll may be the service restarting `--server`.
        assert_eq!(watcher_action(Mode::InstalledStopped, false, 1), Nothing);
        assert_eq!(watcher_action(Mode::InstalledStopped, false, 2), HostInProcess);
        assert_eq!(watcher_action(Mode::Portable, false, 3), HostInProcess);
        assert_eq!(watcher_action(Mode::InstalledStopped, true, 9), Nothing);
    }

    #[test]
    fn status_off_windows_is_portable_unsupported() {
        if cfg!(windows) {
            return;
        }
        let st = status();
        assert_eq!(st.mode, Mode::Portable);
        assert!(!st.platform_supported);
        assert!(!is_service_mode());
        assert!(install_permanent().is_err());
        assert!(uninstall_permanent().is_err());
    }

    #[test]
    fn status_serializes_for_the_ui() {
        let st = ServiceStatus { mode: Mode::InstalledStopped, platform_supported: true };
        let v = serde_json::to_value(st).unwrap();
        assert_eq!(v["mode"], "installed-stopped");
        assert_eq!(v["platform_supported"], true);
    }

    #[test]
    fn relaunch_script_quotes_and_waits() {
        let exe = r"C:\Program Files\EveryDesk\EveryDesk.exe";
        let s = relaunch_script("EveryDesk", exe, 4242, WaitFor::ServiceRunning).unwrap();
        // Waits for the spawning app to be gone first, every mode.
        assert!(s.contains("tasklist /FI \"PID eq 4242\" /NH | find \" 4242 \""), "{s}");
        assert!(s.contains("sc query \"EveryDesk\" | find \"RUNNING\""), "{s}");
        assert!(s.contains(&format!("if not errorlevel 1 if exist \"{exe}\" goto launch")), "{s}");
        assert!(s.contains(&format!("start \"\" \"{exe}\"")), "{s}");
        // Timeout: delete itself and launch nothing.
        let timeout = s.find("exit /b 0").unwrap();
        assert!(timeout < s.find(":launch").unwrap(), "{s}");
        let s = relaunch_script("EveryDesk", exe, 1, WaitFor::ServiceStopped).unwrap();
        assert!(s.contains("if errorlevel 1 goto launch"), "{s}");
        let s = relaunch_script("EveryDesk", exe, 7, WaitFor::Nothing).unwrap();
        assert!(!s.contains("sc query"), "{s}");
        assert!(s.contains("PID eq 7"), "{s}");
        assert!(s.contains(":launch"));
        // `%` doubled, quotes refused.
        let s = relaunch_script("EveryDesk", r"C:\a%b\E.exe", 1, WaitFor::Nothing).unwrap();
        assert!(s.contains(r#""C:\a%%b\E.exe""#), "{s}");
        assert!(relaunch_script("EveryDesk", "C:\\a\"b.exe", 1, WaitFor::Nothing).is_none());
        assert!(relaunch_script("EveryDesk", "", 1, WaitFor::Nothing).is_none());
    }
}
