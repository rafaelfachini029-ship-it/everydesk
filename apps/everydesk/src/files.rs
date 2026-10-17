//! Sending files to the host over the screen session (spec 5.2, 6.2 moment 5).
//!
//! A drop (or the pill's "Arquivos" picker) becomes one engine upload job per path, written
//! to the host's Desktop, or Downloads when there is no Desktop (OneDrive moves it), or the
//! home folder. The host's home comes from its listing after login (`remote_home`); whether
//! Desktop / Downloads exist is asked by listing each candidate (`read_remote_dir`): a
//! `session:folder` with that exact path means yes, the engine's read-dir error (`session:job`
//! with id 0, file_num -1) means no. The answer is cached per session.
//!
//! Commands: `session_send_files(paths)`, `session_pick_and_send()`, `session_cancel_job(id)`,
//! `session_override_answer(id, file_num, overwrite, remember)`.
//! Events (new): `session:job_added {id, name, to, total, preparing}` — first right after the
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

/// Folder that receives the files: Desktop, else Downloads, else home.
pub fn base_dir(home: &str, sep: char, has_desktop: bool, has_downloads: bool) -> String {
    if has_desktop {
        join(home, sep, "Desktop")
    } else if has_downloads {
        join(home, sep, "Downloads")
    } else {
        home.to_owned()
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
) -> String {
    let sep = if platform == "Windows" { '\\' } else { sep };
    join(&base_dir(home, sep, has_desktop, has_downloads), sep, file_name)
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
    log::info!(
        "everydesk: files go to {}",
        base_dir(&dest.home, dest.sep, dest.has_desktop, dest.has_downloads)
    );
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
            JobAdded { id, name: local.clone(), to: String::new(), total: 0, preparing: true },
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
    if !file_enabled(&s) {
        return Err(dropped(ERR_DENIED.to_owned()));
    }
    let d = resolve(gen, &s).map_err(dropped)?;
    for (id, path, name) in jobs {
        if take_cancelled(id) {
            continue;
        }
        let to = remote_target(&d.home, d.sep, &d.platform, d.has_desktop, d.has_downloads, &name);
        let total = local_size(Path::new(&path));
        s.ui_handler.emit(
            "session:job_added",
            JobAdded { id, name, to: to.clone(), total, preparing: false },
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

/// Sends dropped files/folders to the host. Returns the job ids, in order.
#[tauri::command]
pub async fn session_send_files(paths: Vec<String>) -> Result<Vec<i32>, String> {
    blocking(move || send_paths(paths)).await
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
            remote_target(r"C:\Users\x", '\\', "Windows", true, true, "a.txt"),
            r"C:\Users\x\Desktop\a.txt"
        );
        // the platform wins over a wrong separator
        assert_eq!(
            remote_target(r"C:\Users\x", '/', "Windows", true, false, "a.txt"),
            r"C:\Users\x\Desktop\a.txt"
        );
    }

    #[test]
    fn windows_onedrive_without_desktop_goes_to_downloads() {
        assert_eq!(
            remote_target(r"C:\Users\x", '\\', "Windows", false, true, "foto.jpg"),
            r"C:\Users\x\Downloads\foto.jpg"
        );
        assert_eq!(
            remote_target(r"C:\Users\x\", '\\', "Windows", false, false, "foto.jpg"),
            r"C:\Users\x\foto.jpg"
        );
    }

    #[test]
    fn unix_targets() {
        assert_eq!(
            remote_target("/home/x", '/', "Linux", true, true, "a.txt"),
            "/home/x/Desktop/a.txt"
        );
        assert_eq!(
            remote_target("/Users/x/", '/', "Mac OS", false, true, "a.txt"),
            "/Users/x/Downloads/a.txt"
        );
        assert_eq!(remote_target("/", '/', "Linux", false, false, "a"), "/a");
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
