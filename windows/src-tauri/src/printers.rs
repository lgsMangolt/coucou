// The Printers pill: the printers installed in Windows, looked at every 30 seconds while the pill
// is on, and a warning when one stops (offline, out of paper or toner, a jam, a door open, a job
// stuck in its queue).
//
// What Coucou knows is what Windows knows: the state the printer's driver reports and the print
// queue. A network printer that is switched off but still looks "ready" to Windows is not seen
// here. A problem counts once two looks in a row see it, so a printer waking up is not an alarm,
// and it warns once per problem, not every 30 seconds. A printer can be ignored from the card.
//
// The look is one read-only PowerShell query (WMI's Win32_Printer and Win32_PrintJob), without a
// console window. Printer names stay on this computer; nothing is sent anywhere.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

pub const PILL_ID: &str = "integration_printers";
const EVENT: &str = "printers";
const ALERT_EVENT: &str = "printers-alert";
const POLL: Duration = Duration::from_secs(30);
#[cfg(windows)]
const QUERY_TIMEOUT: Duration = Duration::from_secs(25);
/// A problem is real once this many looks in a row see it.
const CONFIRM: u32 = 2;
const MAX_IGNORED: usize = 100;

#[cfg(windows)]
const SCRIPT: &str = r#"$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$p = @(Get-CimInstance Win32_Printer | Select-Object Name, PrinterStatus, DetectedErrorState, WorkOffline, PortName)
$j = @(Get-CimInstance Win32_PrintJob | Select-Object Name, Status, JobStatus)
ConvertTo-Json -InputObject @{ printers = $p; jobs = $j } -Compress -Depth 4"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Problem {
    Offline,
    PaperOut,
    PaperJam,
    TonerOut,
    DoorOpen,
    Service,
    BinFull,
    Error,
    StuckJob,
}

/// The problem, as the owner's WhatsApp warning says it ("a impressora X está …").
pub fn problem_pt(problem: Problem) -> &'static str {
    match problem {
        Problem::Offline => "desligada ou desconectada",
        Problem::PaperOut => "sem papel",
        Problem::PaperJam => "com papel atolado",
        Problem::TonerOut => "sem toner ou tinta",
        Problem::DoorOpen => "com a porta aberta",
        Problem::Service => "pedindo manutenção",
        Problem::BinFull => "com a bandeja de saída cheia",
        Problem::Error => "com erro",
        Problem::StuckJob => "com um trabalho travado na fila",
    }
}

// ── What Windows says ─────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct RawPrinter {
    name: String,
    printer_status: Option<u32>,
    detected_error_state: Option<u32>,
    work_offline: Option<bool>,
    port_name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct RawJob {
    name: Option<String>,
    status: Option<String>,
    job_status: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Raw {
    printers: Vec<RawPrinter>,
    jobs: Vec<RawJob>,
}

fn parse(text: &str) -> Result<Raw, String> {
    serde_json::from_str::<Raw>(text.trim().trim_start_matches('\u{feff}')).map_err(|e| e.to_string())
}

/// Win32_Printer's PrinterStatus and DetectedErrorState, read as a problem (or none).
fn classify(p: &RawPrinter) -> Option<Problem> {
    let status = p.printer_status.unwrap_or(0);
    let error = p.detected_error_state.unwrap_or(0);
    if p.work_offline.unwrap_or(false) || status == 7 || status == 11 || error == 9 {
        return Some(Problem::Offline);
    }
    match error {
        4 => return Some(Problem::PaperOut),
        8 => return Some(Problem::PaperJam),
        6 => return Some(Problem::TonerOut),
        7 => return Some(Problem::DoorOpen),
        10 => return Some(Problem::Service),
        11 => return Some(Problem::BinFull),
        _ => {}
    }
    if status == 9 || status == 6 {
        return Some(Problem::Error);
    }
    None
}

/// PDF, XPS, OneNote and fax "printers" are programs, not machines.
fn is_virtual(name: &str, port: &str) -> bool {
    let name = name.to_lowercase();
    let port = port.to_uppercase();
    ["pdf", "xps", "onenote", "fax", "send to", "document writer", "virtual"].iter().any(|k| name.contains(k))
        || ["PORTPROMPT:", "NUL:", "FILE:", "SHRFAX:", "XPSPORT:"].iter().any(|p| port.starts_with(p))
}

/// The printer a stuck job waits in: Win32_PrintJob names a job "Printer name, 12".
fn stuck_job_printer(job: &RawJob) -> Option<String> {
    let text = format!("{} {}", job.status.as_deref().unwrap_or(""), job.job_status.as_deref().unwrap_or("")).to_lowercase();
    if !["error", "paperout", "offline", "blocked", "intervention"].iter().any(|k| text.contains(k)) {
        return None;
    }
    job.name.as_deref()?.rsplit_once(", ").map(|(printer, _)| printer.to_string())
}

// ── What the island shows ─────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterView {
    pub name: String,
    pub problem: Option<Problem>,
    pub ignored: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// The pill is on and looking.
    pub on: bool,
    /// At least one look has been made.
    pub checked: bool,
    /// The last look could not read Windows' printers.
    pub failed: bool,
    /// Problems first, then by name.
    pub printers: Vec<PrinterView>,
    /// Printers with a problem that are not ignored.
    pub problems: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub name: String,
    pub problem: Problem,
}

#[derive(Default)]
struct Inner {
    ignored: HashSet<String>,
    report: Report,
    /// Per printer: the problem the last looks saw, and how many looks in a row.
    streaks: HashMap<String, (Option<Problem>, u32)>,
    /// The problem each printer was already warned about.
    warned: HashMap<String, Problem>,
    /// Printers with at least one job in their queue at the last look.
    busy: HashSet<String>,
}

fn recount(report: &mut Report) {
    report.problems = report.printers.iter().filter(|p| p.problem.is_some() && !p.ignored).count() as u32;
}

/// One look: the report, and the warnings that are new.
fn analyze(raw: Raw, inner: &mut Inner) -> (Report, Vec<Alert>) {
    let mut stuck: HashSet<String> = HashSet::new();
    for job in &raw.jobs {
        if let Some(printer) = stuck_job_printer(job) {
            stuck.insert(printer);
        }
    }
    inner.busy = raw.jobs.iter().filter_map(|job| job.name.as_deref()?.rsplit_once(", ").map(|(printer, _)| printer.trim().to_string())).collect();
    let mut views = Vec::new();
    let mut alerts = Vec::new();
    let mut seen = HashSet::new();
    for printer in &raw.printers {
        let name = printer.name.trim().to_string();
        if name.is_empty() || is_virtual(&name, printer.port_name.as_deref().unwrap_or("")) {
            continue;
        }
        seen.insert(name.clone());
        let observed = classify(printer).or_else(|| stuck.contains(&name).then_some(Problem::StuckJob));
        let streak = inner.streaks.entry(name.clone()).or_insert((None, 0));
        *streak = match (observed, streak.0) {
            (Some(now), Some(before)) if now == before => (Some(now), streak.1 + 1),
            (Some(now), _) => (Some(now), 1),
            (None, _) => (None, 0),
        };
        let confirmed = streak.0.filter(|_| streak.1 >= CONFIRM);
        let ignored = inner.ignored.contains(&name);
        match confirmed {
            Some(problem) => {
                if !ignored && inner.warned.get(&name) != Some(&problem) {
                    alerts.push(Alert { name: name.clone(), problem });
                }
                inner.warned.insert(name.clone(), problem);
            }
            None if observed.is_none() => {
                inner.warned.remove(&name);
            }
            None => {}
        }
        views.push(PrinterView { name, problem: confirmed, ignored });
    }
    inner.streaks.retain(|name, _| seen.contains(name));
    inner.warned.retain(|name, _| seen.contains(name));
    views.sort_by(|a, b| b.problem.is_some().cmp(&a.problem.is_some()).then_with(|| a.name.cmp(&b.name)));
    let mut report = Report { on: true, checked: true, failed: false, printers: views, problems: 0 };
    recount(&mut report);
    (report, alerts)
}

// ── The look ──────────────────────────────────────────────────────────────────

static INNER: LazyLock<Mutex<Inner>> = LazyLock::new(|| Mutex::new(Inner { ignored: load_ignored(), ..Default::default() }));
/// The running loop's number; 0 when there is none.
static LOOP: AtomicU64 = AtomicU64::new(0);
static NEXT: AtomicU64 = AtomicU64::new(1);

fn inner() -> MutexGuard<'static, Inner> {
    INNER.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The printers Windows knows (as of the last look; none while the pill is off), and the ones
/// that are printing: what a print order picks its printer from (whatsapp/orders.rs).
pub fn queue() -> (Vec<String>, HashSet<String>) {
    let inner = inner();
    (inner.report.printers.iter().map(|p| p.name.clone()).collect(), inner.busy.clone())
}

fn ignored_path() -> PathBuf {
    crate::platform::config_dir().join("printers.json")
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct IgnoredFile {
    ignored: Vec<String>,
}

fn load_ignored() -> HashSet<String> {
    std::fs::read(ignored_path())
        .ok()
        .and_then(|b| serde_json::from_slice::<IgnoredFile>(&b).ok())
        .map(|f| f.ignored.into_iter().take(MAX_IGNORED).collect())
        .unwrap_or_default()
}

fn save_ignored(set: &HashSet<String>) {
    let path = ignored_path();
    let Some(dir) = path.parent() else { return };
    let mut names: Vec<String> = set.iter().cloned().collect();
    names.sort();
    let temp = path.with_extension(format!("json.coucou-{}", std::process::id()));
    let written = crate::platform::ensure_private_dir(dir)
        .and_then(|()| serde_json::to_vec_pretty(&IgnoredFile { ignored: names }).map_err(std::io::Error::other))
        .and_then(|json| std::fs::write(&temp, json))
        .and_then(|()| std::fs::rename(&temp, &path));
    if let Err(err) = written {
        let _ = std::fs::remove_file(&temp);
        crate::log::line(format!("printers: could not save printers.json: {err}"));
    }
}

#[cfg(windows)]
async fn query() -> Result<Raw, String> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", SCRIPT]).stdin(Stdio::null());
    crate::platform::no_console(&mut cmd);
    let mut cmd = tokio::process::Command::from(cmd);
    cmd.kill_on_drop(true);
    let out = tokio::time::timeout(QUERY_TIMEOUT, cmd.output())
        .await
        .map_err(|_| "timed out".to_string())?
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("powershell exited with {}", out.status));
    }
    parse(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(windows))]
async fn query() -> Result<Raw, String> {
    Err("Windows only".into())
}

fn publish(app: &AppHandle, report: &Report) {
    let _ = app.emit_to(crate::island::WINDOW_LABEL, EVENT, report.clone());
}

/// One look now: the report, and a warning for each printer that just stopped.
async fn look(app: &AppHandle) -> Report {
    let result = query().await;
    let (report, alerts) = {
        let mut i = inner();
        match result {
            Ok(raw) => {
                let (report, alerts) = analyze(raw, &mut i);
                i.report = report.clone();
                (report, alerts)
            }
            Err(reason) => {
                crate::log::line(format!("printers: could not read the printers: {reason}"));
                i.report.on = true;
                i.report.checked = true;
                i.report.failed = true;
                (i.report.clone(), Vec::new())
            }
        }
    };
    publish(app, &report);
    for alert in alerts {
        crate::whatsapp::tell_owner(format!("⚠️ Coucou: a impressora {} está {}.", alert.name, problem_pt(alert.problem)));
        let _ = app.emit_to(crate::island::WINDOW_LABEL, ALERT_EVENT, alert);
    }
    report
}

/// The pill is turned on or off (Settings, a mode): the looking follows.
pub fn sync(app: &AppHandle, active_integrations: &[String]) {
    let on = active_integrations.iter().any(|id| id == PILL_ID);
    let running = LOOP.load(Ordering::SeqCst) != 0;
    if on && !running {
        let generation = NEXT.fetch_add(1, Ordering::SeqCst);
        LOOP.store(generation, Ordering::SeqCst);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            while LOOP.load(Ordering::SeqCst) == generation {
                look(&app).await;
                tokio::time::sleep(POLL).await;
            }
        });
    } else if !on && running {
        LOOP.store(0, Ordering::SeqCst);
        {
            let mut i = inner();
            i.report = Report::default();
            i.streaks.clear();
            i.warned.clear();
        }
        publish(app, &Report::default());
    }
}

fn only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == crate::island::WINDOW_LABEL { Ok(()) } else { Err("not from this window".into()) }
}

#[tauri::command]
pub fn printers_get(window: WebviewWindow) -> Result<Report, String> {
    only(&window)?;
    Ok(inner().report.clone())
}

/// "Check now" on the card.
#[tauri::command]
pub async fn printers_refresh(window: WebviewWindow) -> Result<Report, String> {
    only(&window)?;
    if LOOP.load(Ordering::SeqCst) == 0 {
        return Ok(Report::default());
    }
    Ok(look(window.app_handle()).await)
}

fn set_ignored(window: &WebviewWindow, name: &str, ignore: bool) -> Result<Report, String> {
    only(window)?;
    let report = {
        let mut i = inner();
        if ignore {
            if i.ignored.len() >= MAX_IGNORED {
                return Err("too many ignored printers".into());
            }
            i.ignored.insert(name.to_string());
        } else {
            i.ignored.remove(name);
        }
        save_ignored(&i.ignored);
        let ignored = i.ignored.clone();
        for printer in &mut i.report.printers {
            printer.ignored = ignored.contains(&printer.name);
        }
        recount(&mut i.report);
        i.report.clone()
    };
    publish(window.app_handle(), &report);
    Ok(report)
}

#[tauri::command]
pub fn printers_ignore(window: WebviewWindow, name: String) -> Result<Report, String> {
    set_ignored(&window, &name, true)
}

#[tauri::command]
pub fn printers_unignore(window: WebviewWindow, name: String) -> Result<Report, String> {
    set_ignored(&window, &name, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn printer(name: &str, status: u32, error: u32) -> RawPrinter {
        RawPrinter {
            name: name.into(),
            printer_status: Some(status),
            detected_error_state: Some(error),
            work_offline: Some(false),
            port_name: Some("IP_192.168.0.9".into()),
        }
    }

    fn raw(printers: Vec<RawPrinter>) -> Raw {
        Raw { printers, jobs: vec![] }
    }

    #[test]
    fn windows_states_become_problems() {
        assert_eq!(classify(&printer("A", 3, 2)), None, "idle, no error");
        assert_eq!(classify(&printer("A", 4, 2)), None, "printing");
        assert_eq!(classify(&printer("A", 3, 3)), None, "low paper is not a stop");
        assert_eq!(classify(&printer("A", 7, 2)), Some(Problem::Offline));
        assert_eq!(classify(&printer("A", 3, 9)), Some(Problem::Offline));
        assert_eq!(classify(&printer("A", 3, 4)), Some(Problem::PaperOut));
        assert_eq!(classify(&printer("A", 3, 8)), Some(Problem::PaperJam));
        assert_eq!(classify(&printer("A", 3, 6)), Some(Problem::TonerOut));
        assert_eq!(classify(&printer("A", 3, 7)), Some(Problem::DoorOpen));
        assert_eq!(classify(&printer("A", 9, 2)), Some(Problem::Error));
        let mut offline = printer("A", 3, 2);
        offline.work_offline = Some(true);
        assert_eq!(classify(&offline), Some(Problem::Offline));
    }

    #[test]
    fn programs_that_look_like_printers_are_left_out() {
        assert!(is_virtual("Microsoft Print to PDF", "PORTPROMPT:"));
        assert!(is_virtual("OneNote (Desktop)", "nul:"));
        assert!(is_virtual("Fax", "SHRFAX:"));
        assert!(!is_virtual("Epson L3250", "IP_192.168.0.9"));
        assert!(!is_virtual("HP LaserJet", "USB001"));
    }

    #[test]
    fn a_stuck_job_names_its_printer_and_a_busy_one_is_not_stuck() {
        let job = |name: &str, status: &str| RawJob { name: Some(name.into()), status: Some(status.into()), job_status: None };
        assert_eq!(stuck_job_printer(&job("Epson L3250, 12", "Error - Printing")), Some("Epson L3250".into()));
        assert_eq!(stuck_job_printer(&job("HP, Inc LaserJet, 3", "Paperout")), Some("HP, Inc LaserJet".into()));
        assert_eq!(stuck_job_printer(&job("Epson L3250, 12", "Printing")), None);
        assert_eq!(stuck_job_printer(&job("Epson L3250, 12", "Spooling")), None);
    }

    #[test]
    fn a_problem_counts_after_two_looks_and_warns_once() {
        let mut i = Inner::default();
        let bad = || raw(vec![printer("Epson", 3, 4), printer("HP", 3, 2)]);
        let (first, alerts) = analyze(bad(), &mut i);
        assert!(alerts.is_empty() && first.problems == 0, "one look is not enough");
        let (second, alerts) = analyze(bad(), &mut i);
        assert_eq!(second.problems, 1);
        assert_eq!(alerts, vec![Alert { name: "Epson".into(), problem: Problem::PaperOut }]);
        assert_eq!(second.printers[0].name, "Epson", "problems first");
        let (_, alerts) = analyze(bad(), &mut i);
        assert!(alerts.is_empty(), "warned once, not every look");
    }

    #[test]
    fn a_printer_that_recovers_warns_again_when_it_stops_again() {
        let mut i = Inner::default();
        let stopped = || raw(vec![printer("Epson", 7, 2)]);
        let fine = || raw(vec![printer("Epson", 3, 2)]);
        analyze(stopped(), &mut i);
        assert_eq!(analyze(stopped(), &mut i).1.len(), 1);
        let (report, _) = analyze(fine(), &mut i);
        assert_eq!(report.problems, 0);
        analyze(stopped(), &mut i);
        assert_eq!(analyze(stopped(), &mut i).1.len(), 1);
    }

    #[test]
    fn a_change_of_problem_is_a_new_warning_and_an_ignored_printer_stays_quiet() {
        let mut i = Inner::default();
        analyze(raw(vec![printer("Epson", 3, 4)]), &mut i);
        assert_eq!(analyze(raw(vec![printer("Epson", 3, 4)]), &mut i).1.len(), 1);
        analyze(raw(vec![printer("Epson", 3, 8)]), &mut i);
        let (_, alerts) = analyze(raw(vec![printer("Epson", 3, 8)]), &mut i);
        assert_eq!(alerts[0].problem, Problem::PaperJam);

        let mut quiet = Inner::default();
        quiet.ignored.insert("Old".into());
        analyze(raw(vec![printer("Old", 7, 2)]), &mut quiet);
        let (report, alerts) = analyze(raw(vec![printer("Old", 7, 2)]), &mut quiet);
        assert!(alerts.is_empty());
        assert_eq!((report.problems, report.printers[0].ignored), (0, true));
    }

    #[test]
    fn a_stuck_job_is_a_problem_for_a_printer_that_looks_fine() {
        let mut i = Inner::default();
        let jobs = || vec![RawJob { name: Some("Epson, 4".into()), status: Some("Error".into()), job_status: None }];
        let look = || Raw { printers: vec![printer("Epson", 3, 2)], jobs: jobs() };
        analyze(look(), &mut i);
        let (report, alerts) = analyze(look(), &mut i);
        assert_eq!(report.printers[0].problem, Some(Problem::StuckJob));
        assert_eq!(alerts.len(), 1);
    }

    #[test]
    fn what_powershell_prints_is_read_with_or_without_a_bom() {
        let json = "\u{feff}{\"printers\":[{\"Name\":\"Epson\",\"PrinterStatus\":3,\"DetectedErrorState\":2,\"WorkOffline\":false,\"PortName\":\"USB001\"}],\"jobs\":[]}";
        let r = parse(json).unwrap();
        assert_eq!(r.printers.len(), 1);
        assert_eq!(r.printers[0].name, "Epson");
        assert!(parse("{\"printers\":[],\"jobs\":[]}").unwrap().printers.is_empty());
        assert!(parse("not json").is_err());
    }
}
