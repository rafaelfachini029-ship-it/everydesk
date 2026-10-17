//! Sending files to the host over the screen session (spec 5.2, 6.2 moment 5).
//!
//! A drop (or the pill's "Arquivos" picker) becomes one engine upload job per path, written
//! to the host's Desktop, or Downloads when there is no Desktop (OneDrive moves it). Never the
//! home folder itself: the host refuses uploads outside Desktop/Downloads, so without either
//! the send fails (`ERR_NO_FOLDER`). The host's home comes from its listing after login (`remote_home`); whether
//! Desktop / Downloads exist is asked by listing each candidate (`read_remote_dir`): a
//! `session:folder` with that exact path means yes, the engine's read-dir error (`session:job`
//! with id 0, file_num -1) means no. The answer is cached per session.
//!
//! Commands: `session_send_files(paths)`, `session_pick_and_send()`, `session_cancel_job(id)`,
//! `session_override_answer(id, file_num, overwrite, remember)`.
//! `session_send_files` only takes paths the native drop just handed over (registered from the
//! window's drag-drop event on the Rust side, `register_dropped_paths`, 60 s, cleared on send):
//! the webview can never name an arbitrary local file to upload. The picker registers its own.
//! A dropped folder is walked first: one name the host cannot store refuses the whole drop.
//! Events (new): `session:job_added {id, name, to, total, files, preparing}` — first right after the
//! drop (`preparing: true`, so the trail shows "Preparando…" while the destination is found),
//! then again with the destination and size just before the job starts; `session:job_failed
//! {id, message}` — that file was not sent (a name the host cannot store); `session:jobs_dropped
//! {ids}` — the whole drop was refused (the command's error says why).

use crate::commands;
use crate::session::TauriSessionHandler;
use librustdesk::client::FileManager;
use librustdesk::ui_session_interface::Session;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Listener};

pub const ERR_NO_HOME: &str = "A outra máquina ainda não informou a pasta de destino.";
pub const ERR_DENIED: &str = "A outra máquina não permite receber arquivos.";
pub const ERR_NO_SESSION: &str = "Nenhuma sessão aberta.";
pub const ERR_NO_ANSWER: &str = "A outra máquina não respondeu. Tente de novo.";
pub const ERR_BAD_NAME: &str = "Nome de arquivo inválido para a outra máquina.";
pub const ERR_NO_FOLDER: &str =
    "A outra máquina não tem uma pasta Área de Trabalho ou Downloads para receber arquivos.";
pub const ERR_BAD_TREE: &str = "Um dos arquivos tem um nome que a outra máquina não aceita.";
pub const ERR_NOT_DROPPED: &str = "Arraste os arquivos de novo ou use o botão Arquivos.";
/// How long a native drop's paths may be sent.
pub const DROP_TTL: Duration = Duration::from_secs(60);
/// How long a send waits for its drop to be registered (the webview's drop event and the
/// window event that registers the paths come from the same native event, in either order).
const DROP_WAIT: Duration = Duration::from_millis(500);
/// How long a send waits for the host's home listing (sent once, right after login).
const HOME_WAIT: Duration = Duration::from_secs(3);
/// How long one Desktop/Downloads probe waits. No answer at all = the host ignores file
/// actions from us (permission off or rejected).
const PROBE_WAIT: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(100);

// ---- pure helpers -------------------------------------------------------------------

/// `dir` + `sep` + `name`, without doubling a trailing separator (`/`, `C:\`).
pub fn join(dir: &str, sep: char, name: &str) -> String {
    let d = dir.trim_end_matches(sep);
    format!("{d}{sep}{name}")
}

/// Folder that receives the files: Desktop, else Downloads. The host accepts uploads only
/// there (never the home root), so with neither there is nowhere to send.
pub fn base_dir(home: &str, sep: char, has_desktop: bool, has_downloads: bool) -> Result<String, &'static str> {
    if has_desktop {
        Ok(join(home, sep, "Desktop"))
    } else if has_downloads {
        Ok(join(home, sep, "Downloads"))
    } else {
        Err(ERR_NO_FOLDER)
    }
}

/// Full destination path of `file_name` on the host. Windows hosts always use `\`; the folder
/// is named "Desktop" on disk in every language (pt-BR shows "Área de Trabalho" over it).
pub fn remote_target(
    home: &str,
    sep: char,
    platform: &str,
    has_desktop: bool,
    has_downloads: bool,
    file_name: &str,
) -> Result<String, &'static str> {
    let sep = if platform == "Windows" { '\\' } else { sep };
    Ok(join(&base_dir(home, sep, has_desktop, has_downloads)?, sep, file_name))
}

/// Last component of a local path ("/a/b/" → "b"); None for "/" or "".
pub fn file_name_of(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
}

/// The name `name` gets on the host, or `ERR_BAD_NAME`. A file name is only a name: a path
/// separator of the host (or `.`/`..`) would let a dropped file land outside the destination
/// folder, so it is refused everywhere. On a Windows host also refused: `: * ? " < > |`, control
/// characters and reserved device names (`CON`, `NUL.txt`, `COM1`...); trailing dots and
/// spaces are dropped, as Windows itself would.
pub fn host_safe_name(name: &str, platform: &str) -> Result<String, &'static str> {
    let windows = platform == "Windows";
    let name = if windows { name.trim_end_matches(['.', ' ']) } else { name };
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\0']) {
        return Err(ERR_BAD_NAME);
    }
    if windows {
        if name.chars().any(|c| c.is_control() || "\\:*?\"<>|".contains(c)) {
            return Err(ERR_BAD_NAME);
        }
        let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && matches!(stem.as_bytes()[3], b'1'..=b'9'));
        if reserved {
            return Err(ERR_BAD_NAME);
        }
    } else if name.contains('\\') && platform.is_empty() {
        // unknown platform: be as strict as Windows about separators
        return Err(ERR_BAD_NAME);
    }
    Ok(name.to_owned())
}

/// Files the engine will upload for `path` (`include_hidden = false`, as `send_files` is
/// called), after checking every name inside a folder against the host platform: one bad
/// name refuses the whole drop (`ERR_BAD_TREE`). A plain file counts 1.
pub fn check_tree(path: &Path, platform: &str) -> Result<usize, &'static str> {
    if !path.is_dir() {
        return Ok(1);
    }
    let entries = base::fs::get_recursive_files(&path.to_string_lossy(), false)
        .map_err(|_| ERR_BAD_TREE)?;
    for e in &entries {
        for c in Path::new(&e.name).components() {
            match c {
                std::path::Component::Normal(n) => {
                    host_safe_name(&n.to_string_lossy(), platform).map_err(|_| ERR_BAD_TREE)?;
                }
                _ => return Err(ERR_BAD_TREE),
            }
        }
    }
    Ok(entries.len())
}

/// Paths the native drop (or the picker) handed over, each until `DROP_TTL` passes.
#[derive(Default, Debug)]
pub struct DropRegistry {
    entries: Vec<(String, Instant)>,
}

impl DropRegistry {
    fn prune(&mut self, now: Instant) {
        self.entries
            .retain(|(_, at)| now.saturating_duration_since(*at) < DROP_TTL);
    }

    pub fn register(&mut self, paths: impl IntoIterator<Item = String>, now: Instant) {
        self.prune(now);
        for p in paths {
            self.entries.retain(|(q, _)| *q != p);
            self.entries.push((p, now));
        }
    }

    /// All of `paths` were registered and are still fresh: the registry is cleared (one drop,
    /// one send) and the send may go. Otherwise nothing changes.
    pub fn claim(&mut self, paths: &[String], now: Instant) -> Result<(), &'static str> {
        self.prune(now);
        if paths.iter().all(|p| self.entries.iter().any(|(q, _)| q == p)) {
            self.entries.clear();
            Ok(())
        } else {
            Err(ERR_NOT_DROPPED)
        }
    }
}

static DROPS: Mutex<DropRegistry> = Mutex::new(DropRegistry { entries: Vec::new() });

/// Called from the window's native drag-drop event (lib.rs), never from the webview.
pub fn register_dropped_paths(paths: &[std::path::PathBuf]) {
    let paths: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    log::info!("everydesk: {} path(s) dropped", paths.len());
    DROPS.lock().unwrap().register(paths, Instant::now());
}

fn claim_dropped(paths: &[String]) -> Result<(), String> {
    let deadline = Instant::now() + DROP_WAIT;
    loop {
        match DROPS.lock().unwrap().claim(paths, Instant::now()) {
            Ok(()) => return Ok(()),
            Err(e) if Instant::now() >= deadline => {
                log::warn!("everydesk: send refused: {} path(s) not from a drop", paths.len());
                return Err(e.to_owned());
            }
            Err(_) => {}
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// A probe's outcome: listed (true), the host's read-dir failed (false), or no answer at all
/// (None) — then the host is not answering, which is not the same as refusing.
pub fn probe_result(answer: Option<bool>) -> Result<bool, &'static str> {
    answer.ok_or(ERR_NO_ANSWER)
}

/// Where a send stands before it may start.
#[derive(Debug, PartialEq)]
pub enum HomeCheck {
    Ready(String),
    Wait,
    Fail(&'static str),
}

/// The host turned file permission off (it says so at login and on every switch): refuse at
/// once. Otherwise go as soon as the home folder is known, and give up after the wait.
pub fn home_check(file_enabled: bool, home: Option<String>, timed_out: bool) -> HomeCheck {
    match (file_enabled, home) {
        (false, _) => HomeCheck::Fail(ERR_DENIED),
        (true, Some(h)) => HomeCheck::Ready(h),
        (true, None) if timed_out => HomeCheck::Fail(ERR_NO_HOME),
        (true, None) => HomeCheck::Wait,
    }
}

/// The parts of `session:folder` / `session:job` a probe needs.
#[derive(Debug, Deserialize)]
pub struct FolderSeen {
    pub path: String,
    pub is_local: bool,
    pub only_count: bool,
}
#[derive(Debug, Deserialize)]
pub struct JobSeen {
    pub id: i32,
    pub file_num: i32,
    pub kind: String,
}

pub enum Seen {
    Folder(FolderSeen),
    Job(JobSeen),
}

/// Does this event answer the probe of `probe_path`? Some(true): the host listed it (it
/// exists). Some(false): the host's read-dir failed. The engine's failure does not echo the
/// path (`fs::new_error(0, err, -1)` carries only id 0 and file_num -1; real jobs start at 1),
/// so any read-dir failure while a probe is pending answers it; probes are serialized
/// (`RESOLVING`) and this app issues no other id-0 file action. None: unrelated.
pub fn probe_answer(probe_path: &str, seen: &Seen) -> Option<bool> {
    match seen {
        Seen::Folder(f) if !f.is_local && !f.only_count && f.path == probe_path => Some(true),
        Seen::Job(j) if j.id == 0 && j.file_num == -1 && j.kind == "error" => Some(false),
        _ => None,
    }
}

/// Bytes under `path` (files, recursively; hidden entries below the top skipped as the engine
/// does with `include_hidden = false`; symlinks not followed). Only for the trail's percent.
pub fn local_size(path: &Path) -> u64 {
    fn walk(p: &Path, top: bool) -> u64 {
        let Ok(md) = std::fs::symlink_metadata(p) else { return 0 };
        if !top
            && p.file_name()
                .map_or(false, |n| n.to_string_lossy().starts_with('.'))
        {
            return 0;
        }
        if md.is_file() {
            md.len()
        } else if md.is_dir() {
            std::fs::read_dir(p)
                .map(|rd| rd.flatten().map(|e| walk(&e.path(), false)).sum())
                .unwrap_or(0)
        } else {
            0
        }
    }
    walk(path, true)
}

// ---- per-session state --------------------------------------------------------------

/// Where this session's files go, once resolved.
#[derive(Clone, Debug)]
struct Dest {
    home: String,
    sep: char,
    platform: String,
    has_desktop: bool,
    has_downloads: bool,
}

struct Probe {
    path: String,
    tx: mpsc::Sender<bool>,
}

#[derive(Default)]
struct State {
    /// Session generation `dest` belongs to.
    gen: u64,
    dest: Option<Dest>,
    probe: Option<Probe>,
}

static STATE: Mutex<State> = Mutex::new(State { gen: 0, dest: None, probe: None });
/// One resolution at a time: a drop during the picker waits for the same answer.
static RESOLVING: Mutex<()> = Mutex::new(());

/// Watches the session events that answer a probe. Called once from `setup`.
pub fn start(app: &AppHandle) {
    app.listen_any("session:folder", |e| {
        if let Ok(f) = serde_json::from_str::<FolderSeen>(e.payload()) {
            answer(&Seen::Folder(f));
        }
    });
    app.listen_any("session:job", |e| {
        if let Ok(j) = serde_json::from_str::<JobSeen>(e.payload()) {
            answer(&Seen::Job(j));
        }
    });
}

fn answer(seen: &Seen) {
    let mut st = STATE.lock().unwrap();
    let hit = st.probe.as_ref().and_then(|p| probe_answer(&p.path, seen));
    if let Some(found) = hit {
        if let Some(p) = st.probe.take() {
            let _ = p.tx.send(found);
        }
    }
}

type S = Session<TauriSessionHandler>;

fn file_enabled(s: &S) -> bool {
    *s.server_file_transfer_enabled.read().unwrap()
}

fn wait_home(s: &S) -> Result<String, String> {
    let deadline = Instant::now() + HOME_WAIT;
    loop {
        let timed_out = Instant::now() >= deadline;
        match home_check(file_enabled(s), s.remote_home(), timed_out) {
            HomeCheck::Ready(h) => return Ok(h),
            HomeCheck::Fail(e) => return Err(e.to_owned()),
            HomeCheck::Wait => std::thread::sleep(POLL),
        }
    }
}

/// Lists `path` on the host; Ok(exists).
fn probe(s: &S, path: String) -> Result<bool, String> {
    let (tx, rx) = mpsc::channel();
    STATE.lock().unwrap().probe = Some(Probe { path: path.clone(), tx });
    s.read_remote_dir(path, false);
    let res = rx.recv_timeout(PROBE_WAIT).ok();
    STATE.lock().unwrap().probe = None;
    if res.is_none() {
        log::warn!("everydesk: host did not answer the folder probe");
    }
    probe_result(res).map_err(str::to_owned)
}

/// Ids the person cancelled while they were still "Preparando…": never started.
static CANCELLED: Mutex<Vec<i32>> = Mutex::new(Vec::new());

fn take_cancelled(id: i32) -> bool {
    let mut c = CANCELLED.lock().unwrap();
    match c.iter().position(|&x| x == id) {
        Some(i) => {
            c.swap_remove(i);
            true
        }
        None => false,
    }
}

fn resolve(gen: u64, s: &S) -> Result<Dest, String> {
    let _one = RESOLVING.lock().unwrap();
    {
        let st = STATE.lock().unwrap();
        if st.gen == gen {
            if let Some(d) = &st.dest {
                return Ok(d.clone());
            }
        }
    }
    let home = wait_home(s)?;
    let platform = s.get_platform(true);
    let sep = if platform == "Windows" { '\\' } else { '/' };
    // Answered probes (found or not) are cached for the session below; a timeout is not, so
    // the next drop asks again ("Tente de novo").
    let has_desktop = probe(s, join(&home, sep, "Desktop"))?;
    let has_downloads = !has_desktop && probe(s, join(&home, sep, "Downloads"))?;
    let dest = Dest { home, sep, platform, has_desktop, has_downloads };
    // Neither folder: refused (not cached, so a folder created meanwhile is found next time).
    let base = base_dir(&dest.home, dest.sep, dest.has_desktop, dest.has_downloads)
        .map_err(|e| {
            log::warn!("everydesk: host has no Desktop nor Downloads");
            e.to_owned()
        })?;
    log::info!("everydesk: files go to {base}");
    let mut st = STATE.lock().unwrap();
    st.gen = gen;
    st.dest = Some(dest.clone());
    Ok(dest)
}

#[derive(Clone, Serialize)]
struct JobAdded {
    id: i32,
    name: String,
    to: String,
    total: u64,
    /// Files in the job (a folder's count; 1 for a file; 0 while preparing).
    files: usize,
    preparing: bool,
}

#[derive(Clone, Serialize)]
struct JobFailed {
    id: i32,
    message: &'static str,
}

#[derive(Clone, Serialize)]
struct JobsDropped {
    ids: Vec<i32>,
}

fn send_paths(paths: Vec<String>) -> Result<Vec<i32>, String> {
    let (gen, s) = commands::current().ok_or_else(|| ERR_NO_SESSION.to_owned())?;
    let platform = s.get_platform(true);
    let mut ids = Vec::new();
    // (id, local path, name on the host)
    let mut jobs: Vec<(i32, String, String)> = Vec::new();
    // A row per file at once: finding the destination can take a few seconds.
    for path in paths {
        let Some(local) = file_name_of(&path) else { continue };
        let id = s.get_next_job_id();
        ids.push(id);
        s.ui_handler.emit(
            "session:job_added",
            JobAdded { id, name: local.clone(), to: String::new(), total: 0, files: 0, preparing: true },
        );
        match host_safe_name(&local, &platform) {
            Ok(name) => jobs.push((id, path, name)),
            Err(message) => {
                log::warn!("everydesk: job {id} refused: unsafe name for {platform}");
                s.ui_handler.emit("session:job_failed", JobFailed { id, message });
            }
        }
    }
    if jobs.is_empty() {
        return Ok(ids);
    }
    let dropped = |e: String| {
        let ids = jobs.iter().map(|j| j.0).collect();
        s.ui_handler.emit("session:jobs_dropped", JobsDropped { ids });
        e
    };
    // Names inside folders: one the host cannot store refuses the whole drop.
    let mut counts = Vec::with_capacity(jobs.len());
    for (_, path, _) in &jobs {
        match check_tree(Path::new(path), &platform) {
            Ok(n) => counts.push(n),
            Err(e) => {
                log::warn!("everydesk: drop refused: a name inside a folder is unsafe for {platform}");
                return Err(dropped(e.to_owned()));
            }
        }
    }
    if !file_enabled(&s) {
        return Err(dropped(ERR_DENIED.to_owned()));
    }
    let d = resolve(gen, &s).map_err(dropped)?;
    for ((id, path, name), files) in jobs.into_iter().zip(counts) {
        if take_cancelled(id) {
            continue;
        }
        // `resolve` already refused a host without Desktop/Downloads
        let Ok(to) = remote_target(&d.home, d.sep, &d.platform, d.has_desktop, d.has_downloads, &name)
        else {
            continue;
        };
        let total = local_size(Path::new(&path));
        s.ui_handler.emit(
            "session:job_added",
            JobAdded { id, name, to: to.clone(), total, files, preparing: false },
        );
        log::info!("everydesk: send job {id} ({total} bytes)");
        s.send_files(id, 0, path, to, 0, false, false);
    }
    Ok(ids)
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

/// Sends dropped files/folders to the host. Returns the job ids, in order. Only paths the
/// native drop registered (`register_dropped_paths`) are taken.
#[tauri::command]
pub async fn session_send_files(paths: Vec<String>) -> Result<Vec<i32>, String> {
    blocking(move || {
        claim_dropped(&paths)?;
        send_paths(paths)
    })
    .await
}

/// "Arquivos" in the pill: the system picker, then the same send. Cancelled → no jobs.
#[tauri::command]
pub async fn session_pick_and_send(app: AppHandle) -> Result<Vec<i32>, String> {
    use tauri_plugin_dialog::DialogExt;
    blocking(move || {
        let picked = app
            .dialog()
            .file()
            .set_title("Enviar arquivos")
            .blocking_pick_files();
        let paths: Vec<String> = picked
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| p.into_path().ok())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        // the picker's own answer: registered and claimed like a drop
        let mut reg = DROPS.lock().unwrap();
        reg.register(paths.iter().cloned(), Instant::now());
        reg.claim(&paths, Instant::now()).map_err(str::to_owned)?;
        drop(reg);
        send_paths(paths)
    })
    .await
}

#[tauri::command]
pub fn session_cancel_job(id: i32) {
    // still preparing: the send loop skips it (bounded: the loop takes it, or it is one stale id)
    {
        let mut c = CANCELLED.lock().unwrap();
        if c.len() >= 256 {
            c.clear();
        }
        c.push(id);
    }
    if let Some((_, s)) = commands::current() {
        s.cancel_job(id);
    }
}

/// Answers `session:override`: `overwrite` replaces the host's file, false skips only that
/// file (the job goes on); `remember` applies the answer to the rest of the job.
#[tauri::command]
pub fn session_override_answer(id: i32, file_num: i32, overwrite: bool, remember: bool) {
    if let Some((_, s)) = commands::current() {
        s.set_confirm_override_file(id, file_num, overwrite, remember, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_desktop() {
        assert_eq!(
            remote_target(r"C:\Users\x", '\\', "Windows", true, true, "a.txt").unwrap(),
            r"C:\Users\x\Desktop\a.txt"
        );
        // the platform wins over a wrong separator
        assert_eq!(
            remote_target(r"C:\Users\x", '/', "Windows", true, false, "a.txt").unwrap(),
            r"C:\Users\x\Desktop\a.txt"
        );
    }

    #[test]
    fn windows_onedrive_without_desktop_goes_to_downloads() {
        assert_eq!(
            remote_target(r"C:\Users\x", '\\', "Windows", false, true, "foto.jpg").unwrap(),
            r"C:\Users\x\Downloads\foto.jpg"
        );
    }

    /// The host takes uploads only in Desktop/Downloads: without both, no home-root fallback.
    #[test]
    fn no_desktop_nor_downloads_is_refused() {
        assert_eq!(
            remote_target(r"C:\Users\x\", '\\', "Windows", false, false, "foto.jpg"),
            Err(ERR_NO_FOLDER)
        );
        assert_eq!(remote_target("/", '/', "Linux", false, false, "a"), Err(ERR_NO_FOLDER));
        assert_eq!(base_dir("/home/x", '/', false, false), Err(ERR_NO_FOLDER));
        assert_eq!(
            ERR_NO_FOLDER,
            "A outra máquina não tem uma pasta Área de Trabalho ou Downloads para receber arquivos."
        );
    }

    #[test]
    fn unix_targets() {
        assert_eq!(
            remote_target("/home/x", '/', "Linux", true, true, "a.txt").unwrap(),
            "/home/x/Desktop/a.txt"
        );
        assert_eq!(
            remote_target("/Users/x/", '/', "Mac OS", false, true, "a.txt").unwrap(),
            "/Users/x/Downloads/a.txt"
        );
    }

    #[test]
    fn names_of_dropped_paths() {
        assert_eq!(file_name_of("/a/b/c.txt").as_deref(), Some("c.txt"));
        assert_eq!(file_name_of("/a/pasta/").as_deref(), Some("pasta"));
        assert_eq!(file_name_of("/"), None);
        assert_eq!(file_name_of(""), None);
    }

    /// Review Focus 1: file permission off → refused at once with the pill's message.
    #[test]
    fn permission_off_refuses_without_waiting() {
        assert_eq!(home_check(false, None, false), HomeCheck::Fail(ERR_DENIED));
        assert_eq!(home_check(false, Some("/h".into()), false), HomeCheck::Fail(ERR_DENIED));
        assert_eq!(ERR_DENIED, "A outra máquina não permite receber arquivos.");
    }

    #[test]
    fn safe_names_on_windows() {
        let w = |n| host_safe_name(n, "Windows");
        assert_eq!(w("normal.txt"), Ok("normal.txt".into()));
        assert_eq!(w(r"..\..\x.dll"), Err(ERR_BAD_NAME));
        assert_eq!(w("a:b"), Err(ERR_BAD_NAME));
        for bad in ["a/b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", "a\u{1}b", "..", ".", "", " . "] {
            assert_eq!(w(bad), Err(ERR_BAD_NAME), "{bad:?}");
        }
        for dev in ["CON", "con.txt", "Nul", "PRN.log", "aux", "COM1", "lpt9.txt", "COM1 .txt"] {
            assert_eq!(w(dev), Err(ERR_BAD_NAME), "{dev:?}");
        }
        for ok in ["COM0", "COM10", "LPT", "console.txt", "nulo.txt", "relatório 2026.pdf"] {
            assert!(w(ok).is_ok(), "{ok:?}");
        }
        assert_eq!(w("nota. . "), Ok("nota".into())); // trailing dots/spaces dropped
    }

    #[test]
    fn safe_names_elsewhere() {
        let m = |n| host_safe_name(n, "Mac OS");
        assert_eq!(m("normal.txt"), Ok("normal.txt".into()));
        assert_eq!(m("a:b"), Ok("a:b".into())); // legal on macOS/Linux
        assert_eq!(m("../x"), Err(ERR_BAD_NAME));
        assert_eq!(m(".."), Err(ERR_BAD_NAME));
        assert_eq!(m("a\0b"), Err(ERR_BAD_NAME));
        assert_eq!(host_safe_name("a/b", "Linux"), Err(ERR_BAD_NAME));
        // unknown host platform: no backslash either
        assert_eq!(host_safe_name(r"..\x", ""), Err(ERR_BAD_NAME));
    }

    #[test]
    fn probe_timeout_is_not_a_refusal() {
        assert_eq!(probe_result(Some(true)), Ok(true));
        assert_eq!(probe_result(Some(false)), Ok(false));
        assert_eq!(probe_result(None), Err(ERR_NO_ANSWER));
        assert_ne!(ERR_NO_ANSWER, ERR_DENIED);
        assert_eq!(ERR_NO_ANSWER, "A outra máquina não respondeu. Tente de novo.");
    }

    #[test]
    fn home_wait_then_give_up() {
        assert_eq!(home_check(true, None, false), HomeCheck::Wait);
        assert_eq!(home_check(true, None, true), HomeCheck::Fail(ERR_NO_HOME));
        assert_eq!(home_check(true, Some("/h".into()), true), HomeCheck::Ready("/h".into()));
    }

    #[test]
    fn probe_answers() {
        let folder = |path: &str, is_local, only_count| {
            Seen::Folder(FolderSeen { path: path.into(), is_local, only_count })
        };
        let job = |id, file_num, kind: &str| Seen::Job(JobSeen { id, file_num, kind: kind.into() });
        let p = r"C:\Users\x\Desktop";
        assert_eq!(probe_answer(p, &folder(p, false, false)), Some(true));
        assert_eq!(probe_answer(p, &folder(r"C:\Users\x", false, false)), None);
        assert_eq!(probe_answer(p, &folder(p, true, false)), None); // a local listing
        assert_eq!(probe_answer(p, &folder(p, false, true)), None); // a job's count
        assert_eq!(probe_answer(p, &job(0, -1, "error")), Some(false));
        assert_eq!(probe_answer(p, &job(3, 0, "error")), None); // a real job failing
        assert_eq!(probe_answer(p, &job(0, -1, "progress")), None);
    }

    #[test]
    fn event_payloads_parse() {
        let f: FolderSeen = serde_json::from_str(
            r#"{"id":0,"path":"/h/Desktop","is_local":false,"only_count":false,"count":3}"#,
        )
        .unwrap();
        assert_eq!(f.path, "/h/Desktop");
        let j: JobSeen = serde_json::from_str(
            r#"{"id":0,"file_num":-1,"kind":"error","speed":0.0,"finished":0.0,"err":"x"}"#,
        )
        .unwrap();
        assert_eq!((j.id, j.file_num, j.kind.as_str()), (0, -1, "error"));
    }

    fn temp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("everydesk-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// I3: a name inside a dropped folder that the host cannot store refuses the whole drop.
    #[test]
    #[cfg(unix)]
    fn folder_with_a_bad_name_for_windows() {
        let dir = temp("tree-bad");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("ok.txt"), b"x").unwrap();
        std::fs::write(dir.join("sub").join(r"a\b.txt"), b"x").unwrap();
        assert_eq!(check_tree(&dir, "Windows"), Err(ERR_BAD_TREE));
        assert_eq!(check_tree(&dir, ""), Err(ERR_BAD_TREE)); // unknown host: strict
        // legal on a macOS / Linux host
        assert_eq!(check_tree(&dir, "Mac OS"), Ok(2));
        assert_eq!(check_tree(&dir, "Linux"), Ok(2));
        assert_eq!(ERR_BAD_TREE, "Um dos arquivos tem um nome que a outra máquina não aceita.");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn folder_with_a_reserved_name_and_clean_folders() {
        let dir = temp("tree-dev");
        std::fs::create_dir_all(dir.join("x")).unwrap();
        std::fs::write(dir.join("x").join("con.txt"), b"x").unwrap();
        assert_eq!(check_tree(&dir, "Windows"), Err(ERR_BAD_TREE));
        assert_eq!(check_tree(&dir, "Linux"), Ok(1));
        std::fs::remove_file(dir.join("x").join("con.txt")).unwrap();
        std::fs::write(dir.join("x").join("nota.txt"), b"x").unwrap();
        std::fs::write(dir.join(".oculto"), b"x").unwrap(); // not sent (include_hidden = false)
        assert_eq!(check_tree(&dir, "Windows"), Ok(1));
        // a plain file counts one
        assert_eq!(check_tree(&dir.join("x").join("nota.txt"), "Windows"), Ok(1));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// I4: only paths a native drop (or the picker) registered may be sent, once, for 60 s.
    #[test]
    fn drop_registry() {
        let t0 = Instant::now();
        let mut r = DropRegistry::default();
        let a = "/Users/x/foto.jpg".to_owned();
        let b = "/Users/x/pasta".to_owned();
        // nothing registered: refused
        assert_eq!(r.claim(&[a.clone()], t0), Err(ERR_NOT_DROPPED));
        r.register([a.clone(), b.clone()], t0);
        // a path the drop did not carry: refused, and the drop stays
        assert_eq!(r.claim(&[a.clone(), "/etc/passwd".into()], t0), Err(ERR_NOT_DROPPED));
        assert_eq!(r.claim(&[a.clone()], t0 + Duration::from_secs(1)), Ok(()));
        // cleared on send: the same paths cannot go twice
        assert_eq!(r.claim(&[b.clone()], t0 + Duration::from_secs(1)), Err(ERR_NOT_DROPPED));
        // expires after DROP_TTL
        r.register([a.clone()], t0);
        assert_eq!(r.claim(&[a.clone()], t0 + DROP_TTL), Err(ERR_NOT_DROPPED));
        r.register([a.clone()], t0);
        assert_eq!(r.claim(&[a.clone()], t0 + DROP_TTL - Duration::from_millis(1)), Ok(()));
        // a re-drop refreshes the clock
        r.register([a.clone()], t0);
        r.register([a.clone()], t0 + Duration::from_secs(50));
        assert_eq!(r.claim(&[a], t0 + Duration::from_secs(100)), Ok(()));
    }

    #[test]
    fn size_of_a_tree() {
        let dir = std::env::temp_dir().join(format!("everydesk-size-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a"), [0u8; 10]).unwrap();
        std::fs::write(dir.join("sub/b"), [0u8; 5]).unwrap();
        std::fs::write(dir.join(".hidden"), [0u8; 100]).unwrap();
        assert_eq!(local_size(&dir), 15);
        assert_eq!(local_size(&dir.join("a")), 10);
        assert_eq!(local_size(&dir.join("nope")), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
