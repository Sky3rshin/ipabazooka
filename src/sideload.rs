//! The real work: sign in to the Apple account, get a free development
//! certificate and profile, re-sign the app and install it on the phone
//! (or save a signed .ipa). Runs on its own thread with a tokio runtime and
//! talks to the window through channels.

use crate::Kind;
use isideload::auth::apple_account::{TwoFactorCallbackParams, TwoFactorCallbackResponse};
use isideload::auth::builder::AppleAccountBuilder;
use isideload::dev::developer_session::DeveloperSession;
use isideload::sideload::builder::{
    ExtensionsBehavior, ExtensionsBehaviorSelection, MaxCertsBehavior,
};
use isideload::sideload::SideloaderBuilder;
use isideload::util::callbacks::{ExtensionsBehaviorCallbackBox, MaxCertsCallbackBox};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub type CodeAnswer = TwoFactorCallbackResponse;

pub struct CodeRequest {
    pub sms: bool,
    pub unknown: bool,
    pub numbers: Vec<(u32, String)>,
    pub last_error: Option<String>,
}

/// One signing certificate already on the Apple ID (shown when the limit is hit).
#[derive(Clone)]
pub struct CertInfo {
    pub serial: String,
    pub name: String,
    pub machine: String,
    pub expires: String,
}

/// The window's answer to "which certificates should be revoked?" (None = cancel).
pub type CertAnswer = Option<Vec<String>>;

pub enum Event {
    Log(Kind, String),
    /// A step's progress: "Step" or "Step: detail", done, total (see progress.rs).
    Progress(String, u64, u64),
    NeedCode(CodeRequest),
    NeedCertChoice(Vec<CertInfo>),
    Finished(Result<String, String>),
}

/// Channels the window uses to answer questions from the running job.
pub struct Answers {
    pub code_tx: tokio::sync::mpsc::UnboundedSender<CodeAnswer>,
    pub cert_tx: tokio::sync::mpsc::UnboundedSender<CertAnswer>,
}

#[derive(Clone, Default)]
pub struct Tweaks {
    pub name: String,
    pub bundle_id: String,
    pub remove_device_limits: bool,
    pub file_sharing: bool,
    pub remove_extensions: bool,
}

impl Tweaks {
    fn any(&self) -> bool {
        !self.name.trim().is_empty()
            || !self.bundle_id.trim().is_empty()
            || self.remove_device_limits
            || self.file_sharing
            || self.remove_extensions
    }
}

pub struct Job {
    pub email: String,
    pub password: String,
    /// Sign-in (anisette) servers to try, in order.
    pub anisette_servers: Vec<String>,
    /// use this PC's iCloud for Windows for sign-in data
    pub local_anisette: LocalAnisette,
    pub ipa: PathBuf,
    pub udid: Option<String>,
    pub export_only: bool,
    pub revoke_certs: bool,
    /// give app extensions the main app's ID instead of their own (saves App IDs)
    pub share_extension_ids: bool,
    /// ask iOS for a higher memory limit (for emulators, heavy apps)
    pub more_memory: bool,
    /// only sign in and report on the developer account (no app, no phone)
    pub check_only: bool,
    pub tweaks: Tweaks,
}

#[derive(Clone)]
pub struct UiLink {
    tx: Sender<Event>,
    ctx: egui::Context,
}

impl UiLink {
    pub fn new(tx: Sender<Event>, ctx: egui::Context) -> Self {
        UiLink { tx, ctx }
    }
    fn send(&self, e: Event) {
        let _ = self.tx.send(e);
        self.ctx.request_repaint();
    }
    fn log(&self, k: Kind, s: impl Into<String>) {
        self.send(Event::Log(k, s.into()));
    }
    /// Reports `done` of `total` for a named step ("Step" or "Step: detail").
    /// A step's total is also its share of the whole bar.
    fn step(&self, msg: impl Into<String>, done: u64, total: u64) {
        self.send(Event::Progress(msg.into(), done, total));
    }
}

// ---------- progress steps (names and sizes are this app's own; MECHANICS.md) ----------

const STEP_PREPARE: &str = "Preparing app";
const STEP_PREPARE_SIZE: u64 = 5;
const STEP_SIGN_IN: &str = "Signing in";
const STEP_SIGN_IN_SIZE: u64 = 10;
const STEP_ACCOUNT: &str = "Developer account";
const STEP_ACCOUNT_SIZE: u64 = 5;
const STEP_SIGN: &str = "Signing";
const STEP_SIGN_SIZE: u64 = 60;
const STEP_INSTALL: &str = "Installing";
const STEP_INSTALL_SIZE: u64 = 100;
const STEP_SAVE: &str = "Saving";
const STEP_SAVE_SIZE: u64 = 20;
const STEP_CERTS: &str = "Certificates";
const STEP_CERTS_SIZE: u64 = 5;
const STEP_APP_IDS: &str = "App IDs";
const STEP_APP_IDS_SIZE: u64 = 5;

/// Tells the bar every step this job will go through (none started yet),
/// so each one's share is known from the beginning.
fn announce_steps(job: &Job, ui: &UiLink) {
    let steps: Vec<(&str, u64)> = if job.check_only {
        vec![
            (STEP_SIGN_IN, STEP_SIGN_IN_SIZE),
            (STEP_ACCOUNT, STEP_ACCOUNT_SIZE),
            (STEP_CERTS, STEP_CERTS_SIZE),
            (STEP_APP_IDS, STEP_APP_IDS_SIZE),
        ]
    } else {
        let mut v = Vec::new();
        if job.tweaks.any() {
            v.push((STEP_PREPARE, STEP_PREPARE_SIZE));
        }
        v.push((STEP_SIGN_IN, STEP_SIGN_IN_SIZE));
        v.push((STEP_ACCOUNT, STEP_ACCOUNT_SIZE));
        v.push((STEP_SIGN, STEP_SIGN_SIZE));
        if job.export_only {
            v.push((STEP_SAVE, STEP_SAVE_SIZE));
        } else {
            v.push((STEP_INSTALL, STEP_INSTALL_SIZE));
        }
        v
    };
    for (name, size) in steps {
        ui.step(name, 0, size);
    }
}

// ---------- library log lines -> our log ----------

static SINK: Mutex<Option<UiLink>> = Mutex::new(None);

struct SinkWriter;
impl Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf).trim().to_string();
        if !text.is_empty() {
            if let Some(ui) = SINK.lock().ok().and_then(|g| g.clone()) {
                if let Some(pct) = text
                    .strip_prefix("Installing: ")
                    .and_then(|r| r.trim_end_matches('%').trim().parse::<f32>().ok())
                {
                    // the progress step keeps one line for this, updated in place
                    let done = (pct.clamp(0.0, 100.0).round() as u64).max(1);
                    ui.step(STEP_INSTALL, done, STEP_INSTALL_SIZE);
                } else {
                    ui.log(Kind::Info, text);
                }
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Detailed library messages go only to the saved log file (tagged DBG).
struct FileDebugWriter;
impl Write for FileDebugWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf).trim().to_string();
        // provisioning messages are long blobs and add nothing for a reader
        // INFO/WARN/ERROR lines already reach the log through the window, so
        // only the extra DEBUG/TRACE detail is added here
        let duplicate = ["INFO", "WARN", "ERROR"].iter().any(|lvl| text.starts_with(lvl));
        if !text.is_empty() && !duplicate && !text.contains("Received provisioning message") {
            crate::write_log_line("DBG ", &text);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn init_logging() {
    use tracing_subscriber::filter::Targets;
    use tracing_subscriber::prelude::*;
    // normal progress lines: shown in the window (and saved)
    let filter = Targets::new().with_target("isideload", tracing::Level::INFO);
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(|| SinkWriter)
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_level(false)
        .with_filter(filter);
    // verbose step-by-step detail: saved log only
    let debug_filter = Targets::new()
        .with_target("isideload", tracing::Level::DEBUG)
        // the phone library's DEBUG output (every USB message) filled the 2 MB
        // log in seconds and pushed out the sign-in history; keep its INFO
        .with_target("idevice", tracing::Level::INFO);
    let debug_layer = tracing_subscriber::fmt::layer()
        .with_writer(|| FileDebugWriter)
        .with_ansi(false)
        .without_time()
        .with_target(true)
        .with_level(true)
        .with_filter(debug_filter);
    let _ = tracing_subscriber::registry().with(layer).with(debug_layer).try_init();
}

/// Turns a library error report (with tree drawing and source-file lines)
/// into plain text: "Failed to log in › Auth error -22406: …". (› rather
/// than an arrow: the window's text font has no arrow and drew a box.)
pub fn clean(e: impl std::fmt::Display) -> String {
    let s = e.to_string();
    let parts: Vec<String> = s
        .lines()
        .map(|l| l.trim_start_matches(|c: char| c.is_whitespace() || "●├│╰─└┬┼╭╮╯".contains(c)).trim().to_string())
        // drop blank lines and the "file.rs:123" source locations
        .filter(|l| !l.is_empty() && !l.contains(".rs:"))
        .collect();
    let mut out: Vec<String> = Vec::new();
    for p in parts {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    if out.is_empty() { s.trim().to_string() } else { out.join(" › ") }
}

/// What to do when Apple's servers can't be reached.
const NETWORK_TIP: &str = "ipabazooka couldn't reach Apple's servers, so your Apple ID wasn't checked at all. \
     This is an internet connection problem, not your Apple ID or password (it tried a few times).\n\
     • Check that the internet works on this PC: open apple.com in a web browser.\n\
     • Turn off any VPN or proxy, then try again.\n\
     • School, work and public Wi-Fi sometimes block Apple sign-in: try another network or a phone hotspot.\n\
     • Let ipabazooka through your firewall or antivirus (its web shield or HTTPS scanning).\n\
     • Wait a minute, then press Start again.";

/// Plain-language advice for errors from Apple's developer service or the phone.
pub fn advice(err: &str) -> Option<&'static str> {
    let l = err.to_lowercase();
    let has = |k: &str| l.contains(&k.to_lowercase());
    Some(if is_network_problem(err) {
        NETWORK_TIP
    } else if has("7460") || has("maximum number of certificates") {
        "Your Apple ID already has the most signing certificates Apple allows (usually made by Sideloadly, AltStore or another computer). \
         When asked, pick an old one to revoke — or tick \"revoke an old certificate\" in Advanced options. \
         Apps signed with a revoked certificate stop opening until you reinstall them."
    } else if has("9401") || has("maximum app id") || has("app id limit") || has("too many app ids") {
        "Free Apple IDs can make only 10 App IDs every 7 days, and this app needs one for itself plus one per extension. \
         In Advanced options, tick \"Remove app extensions\" or \"Share the main app's ID\", or wait a few days."
    } else if has("program license agreement") || has("agreement") || has("not a member") || has("developer program") {
        "This Apple ID hasn't accepted Apple's developer terms. Sign in once at developer.apple.com/account in a web browser, accept the agreement, then try again."
    } else if has("pairingdialogresponsepending") || has("passwordprotected") || has("invalidhostid") || has("pairing") || has("lockdown session") {
        "The phone isn't paired with this computer. Unlock the phone, tap \"Trust\" when asked (enter the passcode), then try again."
    } else if has("mismatchedapplicationidentifierentitlement") || has("applicationidentifier") && has("mismatch") {
        "A copy of this app signed by someone else is already on the phone. Delete it from the phone (or use Phone tools › Apps), or set a different Bundle ID in Advanced options."
    } else if has("deviceosversiontoolow") || has("minimumosversion") {
        "This app needs a newer iOS than the phone has. Tick \"Remove limits on supported devices\" in Advanced options to try anyway."
    } else if has("applicationverificationfailed") || has("verification failed") {
        "The phone rejected the app's signature. Try again; if it keeps happening, revoke old certificates and reinstall."
    } else if has("device limit") || has("maximum number of devices") {
        "This Apple ID has registered the most devices Apple allows this year. Remove old devices at developer.apple.com/account or use another Apple ID."
    } else if has("1100") || has("session") && has("expired") {
        "Apple's session expired. Just press Start again."
    } else if has("no developer teams") {
        "This Apple ID has no developer team. Sign in once at developer.apple.com/account to set up the free team, then try again."
    } else {
        return None;
    })
}

/// Builds the message shown when something fails: what happened, then what to do.
fn failure(prefix: &str, e: impl std::fmt::Display) -> String {
    let raw = e.to_string();
    crate::write_log_line("RAW ", &format!("{prefix} (full report):\n{raw}"));
    let text = clean(&raw);
    match advice(&raw) {
        Some(tip) => format!("{prefix}: {text}\nWhat to do: {tip}"),
        None => format!("{prefix}: {text}"),
    }
}

// ---------- starting a job ----------

/// Starts the job on a background thread. Returns the sender used to answer
/// two-factor prompts.
pub fn spawn(job: Job, ui: UiLink) -> Answers {
    let (code_tx, code_rx) = tokio::sync::mpsc::unbounded_channel();
    let (cert_tx, cert_rx) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        if let Ok(mut g) = SINK.lock() {
            *g = Some(ui.clone());
        }
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build();
        let result = match rt {
            Ok(rt) => rt.block_on(run(job, ui.clone(), code_rx, cert_rx)),
            Err(e) => Err(format!("Couldn't start worker: {e}")),
        };
        if let Ok(mut g) = SINK.lock() {
            *g = None;
        }
        ui.send(Event::Finished(result));
    });
    Answers { code_tx, cert_tx }
}

// ---------- sign-in with retries ----------

/// Public anisette servers. Apple needs this "machine identity" data with
/// every sign-in; when it's stale or rejected Apple answers -22406
/// "Apple ID or password incorrect" even for the right password.
pub const ANISETTE_SERVERS: &[&str] = &[
    "https://ani.iloader.app",
    "https://ani.sidestore.io",
    "https://ani.sidestore.app",
    "https://ani.sidestore.zip",
];

const ANISETTE_STATE_KEY: &str = isideload::anisette::remote_v3::ANISETTE_STATE_KEY;

fn data_dir() -> PathBuf {
    crate::paths::data_dir()
}

fn server_folder(url: &str) -> String {
    let host: String = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect();
    format!("anisette-{host}")
}

/// Keeps the sign-in helper data in plain files under the data folder
/// (%APPDATA%\ipabazooka, or ~/.config/ipabazooka on Linux), one folder per
/// server, so it can be reset. On Linux it also holds the signing
/// certificate's key (in "signing"), since there may be no system keyring;
/// files there are private to the user.
struct FileStorage {
    dir: PathBuf,
}

impl FileStorage {
    fn new(sub: &str) -> Self {
        let dir = data_dir().join(sub);
        let _ = std::fs::create_dir_all(&dir);
        FileStorage { dir }
    }
    fn path(&self, key: &str) -> PathBuf {
        let safe: String = key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
        self.dir.join(safe)
    }
}

impl isideload::util::storage::SideloadingStorage for FileStorage {
    fn store(&self, key: &str, value: &str) -> Result<(), rootcause::Report> {
        crate::paths::write_private(&self.path(key), value.as_bytes())
            .map_err(|e| rootcause::report!("couldn't save {key}: {e}"))?;
        Ok(())
    }
    fn retrieve(&self, key: &str) -> Result<Option<String>, rootcause::Report> {
        match std::fs::read_to_string(self.path(key)) {
            Ok(s) if !s.is_empty() => Ok(Some(s)),
            Ok(_) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(rootcause::report!("couldn't read {key}: {e}")),
        }
    }
    fn delete(&self, key: &str) -> Result<(), rootcause::Report> {
        let _ = std::fs::remove_file(self.path(key));
        Ok(())
    }
}

fn forget_anisette(url: &str) {
    let _ = std::fs::remove_file(FileStorage::new(&server_folder(url)).path(ANISETTE_STATE_KEY));
}

/// Deletes the saved sign-in helper data for every server. Returns how many were removed.
pub fn reset_signin_data() -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(data_dir()) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with("anisette-") {
                let f = e.path().join(ANISETTE_STATE_KEY);
                if std::fs::remove_file(&f).is_ok() {
                    n += 1;
                }
            }
        }
    }
    n
}

#[derive(PartialEq, Debug)]
enum SignInProblem {
    /// Apple said -22406 (wrong Apple ID or password)
    Rejected,
    /// The sign-in helper server didn't work
    Server,
    /// Apple's own servers couldn't be reached (internet, VPN, firewall…):
    /// the Apple ID wasn't checked at all, so try the same thing again
    Network,
    /// Anything else: don't retry
    Other,
}

/// How many times in a row sign-in tries when Apple's servers can't be reached.
const NETWORK_TRIES: u32 = 3;

/// A request to Apple (not to a sign-in helper server) never got an answer.
fn is_network_problem(err: &str) -> bool {
    let l = err.to_lowercase();
    let reached_apple = l.contains("apple.com") || l.contains("url bag");
    let no_answer = l.contains("error sending request")
        || l.contains("failed to fetch url bag")
        || l.contains("tcp connect")
        || l.contains("timed out")
        || l.contains("dns error")
        || l.contains("connection reset")
        || l.contains("connection refused")
        || l.contains("connection closed")
        || l.contains("network is unreachable")
        || l.contains("no such host");
    reached_apple && no_answer
}

fn classify(err: &str) -> SignInProblem {
    let l = err.to_lowercase();
    if l.contains("-22406") {
        // Apple: wrong Apple ID or password
        SignInProblem::Rejected
    } else if is_network_problem(err) {
        SignInProblem::Network
    } else if l.contains("anisette")
        || l.contains("provision")
        || l.contains("client_info")
        || l.contains("get_headers")
        || l.contains("websocket")
        // Apple rejected the sign-in DATA (not the password): -22410 "this action
        // could not be completed", -45061 needs provisioning, -22421. These mean
        // the identity data from this source was no good — try the next source.
        || l.contains("-22410")
        || l.contains("-45061")
        || l.contains("-22421")
        || (l.contains("initial login request") && !l.contains("-22406"))
    {
        SignInProblem::Server
    } else {
        SignInProblem::Other
    }
}

/// The parts of the sign-in advice that depend on where sign-in data comes
/// from: this PC's iCloud for Windows, or (on Linux) the online servers.
#[cfg(windows)]
const SIGNIN_DATA_TIPS: (&str, &str) = (
    "• Apple didn't trust the sign-in data for this account (common with Advanced Data Protection). Install the web \
     versions of iTunes and iCloud for Windows from apple.com (not the Microsoft Store ones) so ipabazooka can use this \
     PC's own sign-in data, and keep \"Sign-in data\" on Automatic in Advanced options.",
    "• Make sure the web versions of iTunes and iCloud for Windows (from apple.com, not the Microsoft Store) are \
     installed, then open iCloud for Windows once and sign in to it with this same Apple ID, so Windows and Apple agree on this device.\n\
     • Make sure the computer's clock is set to the correct time automatically.\n\
     • Then try again. ipabazooka also falls back to its online sign-in servers on its own.",
);
#[cfg(not(windows))]
const SIGNIN_DATA_TIPS: (&str, &str) = (
    "• Apple didn't trust the sign-in data for this account (common with Advanced Data Protection). On Linux this \
     data comes from online sign-in servers: in Advanced options press \"Reset sign-in data\" or pick another \
     \"Online server\", then try again.",
    "• In Advanced options, press \"Reset sign-in data\" or pick another \"Online server\" (on Linux this data \
     comes from online sign-in servers).\n\
     • Make sure the computer's clock is set to the correct time automatically.\n\
     • Then try again.",
);

fn explain(err: &str) -> String {
    let l = err.to_lowercase();
    let (data_22406, data_22410) = SIGNIN_DATA_TIPS;
    let tip: String = if is_network_problem(err) {
        NETWORK_TIP.to_string()
    } else if l.contains("-22406") {
        format!(
        "Apple refused the password for this Apple ID (error -22406). When the password really is right, it's usually one of these:\n\
         {data_22406}\n\
         • The email isn't one Apple accepts for signing in. At account.apple.com › Sign-In and Security › \
         Email & Phone Numbers, use the main address shown there (often an @icloud.com one). Apple also answers \
         \"wrong password\" for an email it doesn't know.\n\
         • An app-specific password was used. Use the normal Apple ID password.\n\
         • Two-factor authentication is off. Turn it on (iPhone: Settings › your name › Sign-In & Security).\n\
         • Security keys are set up on the account. They block sign-in from tools like this; remove them or use another Apple ID.\n\
         • The account needs attention: sign in once at account.apple.com and accept any new terms, \
         or wait if the password was changed in the last day.\n\
         • A typing slip: tick \"Show password\" to check it (Caps Lock, keyboard language).\n\
         Tip: \"Check my Apple ID only\" (next to the Apple ID box) tests sign-in without a phone or app."
        )
    } else if l.contains("-22410") || l.contains("-45061") || l.contains("-22421") {
        format!(
        "Apple didn't accept the sign-in data (\"anisette\"), not your password (error -22410). This means the \
         identity data from this computer or server didn't match what Apple expected:\n\
         {data_22410}"
        )
    } else if l.contains("-20209") || l.contains("locked") {
        "This Apple ID is locked for security reasons. Unlock it at iforgot.apple.com, then try again.".into()
    } else if l.contains("-36607") || l.contains("-20101") {
        "Apple can't sign this account in right now. Wait a while, then try again or pick another sign-in server.".into()
    } else if l.contains("2fa") || l.contains("two-factor") || l.contains("no 2fa code") {
        "The two-factor code step didn't finish. Try again and enter the newest code.".into()
    } else {
        String::new()
    };
    let first = clean(err);
    if tip.is_empty() {
        format!("Sign-in failed: {first}")
    } else {
        format!("Sign-in failed: {first}\n{tip}")
    }
}

/// Masks an email the way the sign-in library does in its own logs: j***e@gmail.com
pub fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((user, domain)) if !user.is_empty() => {
            let first = user.chars().next().unwrap_or('?');
            let last = user.chars().last().unwrap_or('?');
            format!("{first}***{last}@{domain}")
        }
        _ => "***".into(),
    }
}

fn good_signins_path() -> PathBuf {
    data_dir().join("signed_in_ok.txt")
}

/// Remembers (masked) that an Apple ID signed in fine from this computer.
fn remember_good_signin(email: &str) {
    let masked = mask_email(&email.to_lowercase());
    let path = good_signins_path();
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .filter(|l| !l.starts_with(&format!("{masked} ")))
        .collect();
    let today = isideload::anisette::utc_now_iso8601().chars().take(10).collect::<String>();
    lines.push(format!("{masked} {today}"));
    let _ = std::fs::create_dir_all(data_dir());
    let _ = std::fs::write(path, lines.join("\n"));
}

/// Other Apple IDs that signed in fine from this computer: (masked, date).
fn other_good_signins(email: &str) -> Vec<(String, String)> {
    let me = mask_email(&email.to_lowercase());
    std::fs::read_to_string(good_signins_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once(' ').map(|(a, b)| (a.to_string(), b.to_string())))
        .filter(|(a, _)| *a != me)
        .collect()
}

/// Where Apple's sign-in identity data (anisette) comes from.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LocalAnisette {
    /// this PC's iCloud for Windows first, online servers as backup
    Auto,
    /// only this PC's iCloud for Windows
    Only,
    /// only online servers
    Off,
}

#[derive(Clone, Debug)]
enum Source {
    Local,
    Remote(String),
}

impl Source {
    fn label(&self) -> String {
        match self {
            Source::Local => "this PC (iCloud for Windows)".into(),
            Source::Remote(url) => url.trim_start_matches("https://").trim_start_matches("http://").to_string(),
        }
    }
}

/// Signs in. Sign-in data comes from this PC's iCloud for Windows first (like
/// Sideloadly's "iTunes Anisette"), then from online servers. When Apple
/// answers "wrong password" (-22406) it tries, at most three times in total so
/// the account doesn't get locked:
///   1. the email in lower case (Apple IDs aren't case-sensitive),
///   2. the email exactly as typed, if that differs,
///   3. the next sign-in data source (or, online, freshly made data).
/// If a source itself fails (helper or server problem), the next one is used.
async fn sign_in<C>(
    job: &Job,
    ui: &UiLink,
    two_factor: C,
) -> Result<isideload::auth::apple_account::AppleAccount, String>
where
    C: isideload::util::callbacks::TwoFactorCallback + Clone,
{
    let servers: Vec<String> = if job.anisette_servers.is_empty() {
        ANISETTE_SERVERS.iter().map(|s| s.to_string()).collect()
    } else {
        job.anisette_servers.clone()
    };
    let mut sources: Vec<Source> = Vec::new();
    match job.local_anisette {
        LocalAnisette::Auto | LocalAnisette::Only => {
            if let Some(why) = crate::local_anisette::unavailable_reason() {
                ui.log(Kind::Warn, format!("This PC's sign-in data can't be used: {why}."));
                if job.local_anisette == LocalAnisette::Only {
                    return Err(format!(
                        "Sign-in failed: this PC's sign-in data can't be used — {why}.\n\
                         What to do: install the web versions of iTunes and iCloud for Windows from apple.com \
                         (uninstall the Microsoft Store versions first), or choose another sign-in data source in Advanced options."
                    ));
                }
            } else {
                sources.push(Source::Local);
            }
        }
        LocalAnisette::Off => {}
    }
    if job.local_anisette != LocalAnisette::Only {
        sources.extend(servers.into_iter().map(Source::Remote));
    }

    let typed = job.email.trim().to_string();
    let lower = typed.to_lowercase();
    let mut names = vec![lower.clone()];
    if typed != lower {
        names.push(typed.clone());
    }

    let mut rejected = 0;
    let mut unreachable = 0u32;
    let mut last_err = String::new();
    let mut src = 0;
    let mut name = 0;
    let mut reset_done = false;
    while src < sources.len() {
        let source = sources[src].clone();
        let who = &names[name];
        ui.log(Kind::Info, format!("Sign-in try: {} · sign-in data from {}", mask_email(who), source.label()));
        let builder = AppleAccountBuilder::new(who);
        let builder = match &source {
            Source::Local => builder.anisette_provider(crate::local_anisette::LocalAnisetteProvider::new()),
            Source::Remote(url) => builder.anisette_provider(
                isideload::anisette::remote_v3::RemoteV3AnisetteProvider::new(
                    url,
                    Box::new(FileStorage::new(&server_folder(url))),
                    "0".to_string(),
                )
                .map_err(|e| format!("Couldn't set up sign-in: {}", clean(e)))?,
            ),
        };
        let result = builder.login(&job.password, two_factor.clone()).await;
        match result {
            Ok(account) => {
                remember_good_signin(&lower);
                ui.log(Kind::Good, format!("Apple accepted the sign-in (sign-in data from {}).", source.label()));
                if who != &typed {
                    ui.log(Kind::Info, "Signed in using the email in lower case.");
                }
                return Ok(account);
            }
            Err(e) => {
                last_err = e.to_string();
                ui.log(Kind::Info, format!("Apple's answer: {}", clean(&last_err)));
                match classify(&last_err) {
                    SignInProblem::Rejected => {
                        rejected += 1;
                        if rejected >= 3 {
                            break;
                        }
                        if name + 1 < names.len() {
                            name += 1;
                            ui.log(Kind::Warn, "Rejected. Trying the email exactly as typed (capital letters kept)…");
                            continue;
                        }
                        match &source {
                            Source::Local if src + 1 < sources.len() => {
                                src += 1;
                                name = 0;
                                ui.log(Kind::Warn, "Rejected with this PC's sign-in data. Trying an online sign-in server…");
                                continue;
                            }
                            Source::Remote(url) if !reset_done => {
                                reset_done = true;
                                name = 0;
                                forget_anisette(url);
                                ui.log(Kind::Warn, "Rejected. Trying once more with freshly made sign-in data…");
                                continue;
                            }
                            _ => break,
                        }
                    }
                    SignInProblem::Server => {
                        ui.log(Kind::Warn, format!("Sign-in data from {} didn't work. Trying the next source…", source.label()));
                        if let Source::Remote(url) = &source {
                            forget_anisette(url);
                        }
                        src += 1;
                        name = 0;
                        reset_done = false;
                    }
                    SignInProblem::Network => {
                        // Apple never answered, so nothing counted against the
                        // Apple ID: wait a moment and try the same again.
                        unreachable += 1;
                        if unreachable >= NETWORK_TRIES {
                            break;
                        }
                        let wait = 3 * unreachable as u64;
                        ui.log(
                            Kind::Warn,
                            format!(
                                "Couldn't reach Apple's sign-in server (a connection problem, not your Apple ID). \
                                 Trying again in {wait} seconds ({} of {NETWORK_TRIES})…",
                                unreachable + 1
                            ),
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                    }
                    SignInProblem::Other => break,
                }
            }
        }
    }

    let mut msg = explain(&last_err);
    if rejected > 0 {
        let others = other_good_signins(&lower);
        if let Some((who, when)) = others.last() {
            msg = format!(
                "{msg}\nGood to know: {who} signed in fine from this computer ({when}), so ipabazooka and this PC work — \
                 Apple is refusing this particular Apple ID with this sign-in data."
            );
        }
        msg = format!(
            "{msg}\nApple counts every failed try; after several in a row it locks the Apple ID for a while (unlock at iforgot.apple.com)."
        );
    }
    Err(msg)
}

async fn run(
    job: Job,
    ui: UiLink,
    code_rx: tokio::sync::mpsc::UnboundedReceiver<CodeAnswer>,
    cert_rx: tokio::sync::mpsc::UnboundedReceiver<CertAnswer>,
) -> Result<String, String> {
    announce_steps(&job, &ui);

    // 1. apply the chosen tweaks to a copy of the app
    let ipa_path = if job.check_only {
        PathBuf::new()
    } else if job.tweaks.any() {
        ui.step(STEP_PREPARE, 1, STEP_PREPARE_SIZE);
        let p = prepare_ipa(&job.ipa, &job.tweaks).map_err(|e| format!("Couldn't apply options: {e}"))?;
        ui.log(Kind::Info, "Options applied to a working copy of the app.");
        ui.step(STEP_PREPARE, STEP_PREPARE_SIZE, STEP_PREPARE_SIZE);
        p
    } else {
        job.ipa.clone()
    };

    // 2. sign in
    ui.step(STEP_SIGN_IN, 1, STEP_SIGN_IN_SIZE);
    ui.log(Kind::Info, format!("Signing in as {}…", job.email));
    let code_rx = Arc::new(tokio::sync::Mutex::new(code_rx));
    let cb_ui = ui.clone();
    let two_factor = move |p: TwoFactorCallbackParams| {
        let ui = cb_ui.clone();
        let rx = code_rx.clone();
        async move {
            ui.send(Event::NeedCode(CodeRequest {
                sms: p.sms,
                unknown: p.unknown,
                numbers: p
                    .numbers
                    .iter()
                    .map(|n| (n.id, n.number_with_dial_code.clone()))
                    .collect(),
                last_error: p.last_error.clone(),
            }));
            let answer = rx.lock().await.recv().await.unwrap_or(TwoFactorCallbackResponse::Abort);
            Ok::<_, rootcause::Report>(answer)
        }
    };
    let mut account = sign_in(&job, &ui, two_factor).await?;
    drop(job.password);
    ui.log(Kind::Good, "Signed in.");
    ui.step(STEP_SIGN_IN, STEP_SIGN_IN_SIZE, STEP_SIGN_IN_SIZE);

    ui.step(STEP_ACCOUNT, 1, STEP_ACCOUNT_SIZE);
    let dev = DeveloperSession::from_account(&mut account)
        .await
        .map_err(|e| failure("Couldn't open the developer account", e))?;
    ui.log(Kind::Info, "Developer account opened (free Apple ID: apps last 7 days, 10 App IDs per week).");
    ui.step(STEP_ACCOUNT, STEP_ACCOUNT_SIZE, STEP_ACCOUNT_SIZE);
    if job.check_only {
        return account_report(dev, &ui).await;
    }

    // When the Apple ID already has the most certificates, ask which to revoke
    // (or revoke automatically if that option is ticked).
    let cert_rx = Arc::new(tokio::sync::Mutex::new(cert_rx));
    let cert_ui = ui.clone();
    let cert_prompt: MaxCertsCallbackBox = Box::new(move |certs| {
        let ui = cert_ui.clone();
        let rx = cert_rx.clone();
        Box::pin(async move {
            let infos: Vec<CertInfo> = certs
                .iter()
                .filter_map(|c| {
                    Some(CertInfo {
                        serial: c.serial_number.clone()?,
                        name: c.name.clone().unwrap_or_default(),
                        machine: c.machine_name.clone().unwrap_or_default(),
                        expires: c
                            .expiration_date
                            .as_ref()
                            .map(|d| d.to_xml_format().chars().take(10).collect())
                            .unwrap_or_default(),
                    })
                })
                .collect();
            ui.log(
                Kind::Warn,
                format!("Certificate limit reached: this Apple ID already has {} certificate(s). Choose which to revoke.", infos.len()),
            );
            for c in &infos {
                ui.log(Kind::Info, format!("  certificate: {} · made on \"{}\" · expires {}", c.name, c.machine, c.expires));
            }
            ui.send(Event::NeedCertChoice(infos));
            let answer = rx.lock().await.recv().await.flatten();
            match &answer {
                Some(list) => ui.log(Kind::Info, format!("Revoking {} certificate(s)…", list.len())),
                None => ui.log(Kind::Warn, "No certificate chosen; can't make a new one."),
            }
            Ok::<_, rootcause::Report>(answer)
        })
    });

    let extensions = if job.share_extension_ids {
        ExtensionsBehavior::ReuseMain
    } else {
        ExtensionsBehavior::RegisterAll
    };
    ui.log(
        Kind::Info,
        format!(
            "Settings: extensions {}, certificate limit: {}, more memory {}.",
            if job.share_extension_ids { "share the main App ID" } else { "get their own App IDs" },
            if job.revoke_certs { "revoke automatically" } else { "ask me" },
            if job.more_memory { "on" } else { "off" }
        ),
    );
    let builder = SideloaderBuilder::<MaxCertsCallbackBox, ExtensionsBehaviorCallbackBox>::new(
        dev,
        job.email.clone(),
    )
    .machine_name("ipabazooka".to_string())
    .max_certs_behavior(if job.revoke_certs { MaxCertsBehavior::Revoke } else { MaxCertsBehavior::Prompt(cert_prompt) })
    .extensions_behavior(ExtensionsBehaviorSelection::Fixed(extensions));
    // Linux: keep the signing certificate's key in the app's own private folder,
    // because many Linux setups have no system keyring to hold it. (Windows keeps
    // using Windows Credential Manager.) Reusing the key avoids making a new
    // certificate on every install.
    #[cfg(not(windows))]
    let builder = builder.storage(Box::new(FileStorage::new("signing")));
    let mut sideloader = builder.build();

    let pui = ui.clone();
    let progress = move |p: f32| {
        let ui = pui.clone();
        async move {
            let label = if p < 0.15 {
                "Getting certificate"
            } else if p < 0.2 {
                "Registering app ID"
            } else if p < 0.3 {
                "Getting provisioning profile"
            } else {
                "Signing app"
            };
            let done = ((p.clamp(0.0, 1.0) * STEP_SIGN_SIZE as f32).round() as u64).max(1);
            ui.step(format!("{STEP_SIGN}: {label}"), done, STEP_SIGN_SIZE);
        }
    };

    if job.export_only {
        ui.step(format!("{STEP_SIGN}: Getting certificate"), 1, STEP_SIGN_SIZE);
        let (app_dir, _) = sideloader
            .sign_app(ipa_path, None, job.more_memory, Some(progress), None, None)
            .await
            .map_err(|e| failure("Signing failed", e))?;
        ui.step(STEP_SIGN, STEP_SIGN_SIZE, STEP_SIGN_SIZE);
        ui.step(STEP_SAVE, 1, STEP_SAVE_SIZE);
        let out = signed_output_path(&job.ipa);
        zip_app(&app_dir, &out).map_err(|e| format!("Couldn't save the signed .ipa: {e}"))?;
        ui.step(STEP_SAVE, STEP_SAVE_SIZE, STEP_SAVE_SIZE);
        Ok(format!("Saved signed app to {}", out.display()))
    } else {
        let udid = job.udid.clone().ok_or("No device selected")?;
        ui.log(Kind::Info, "Connecting to the device…");
        let mut mux = idevice::usbmuxd::UsbmuxdConnection::default()
            .await
            .map_err(|e| format!("Apple device service not reachable: {e}"))?;
        let device = mux
            .get_device(&udid)
            .await
            .map_err(|e| format!("Device {udid} not found (unplugged?): {e}"))?;
        let provider = device.to_provider(idevice::usbmuxd::UsbmuxdAddr::default(), "ipabazooka");
        ui.log(Kind::Info, format!("Phone: {udid} ({:?})", device.connection_type));
        sideloader
            .install_app(&provider, ipa_path, job.more_memory, Some(progress), None)
            .await
            .map_err(|e| failure("Install failed", e))?;
        ui.step(STEP_SIGN, STEP_SIGN_SIZE, STEP_SIGN_SIZE);
        ui.step(STEP_INSTALL, STEP_INSTALL_SIZE, STEP_INSTALL_SIZE);
        check_developer_mode(&provider, &ui).await;
        Ok("Installed! On the phone: Settings › General › VPN & Device Management › trust your Apple ID, then open the app.".into())
    }
}

/// "Check my Apple ID only": after signing in, read the developer account
/// and report what matters for sideloading.
async fn account_report(mut dev: DeveloperSession, ui: &UiLink) -> Result<String, String> {
    use isideload::dev::app_ids::AppIdsApi;
    use isideload::dev::certificates::CertificatesApi;
    use isideload::dev::teams::TeamsApi;
    let teams = dev
        .list_teams()
        .await
        .map_err(|e| failure("Couldn't read the developer team", e))?;
    let Some(team) = teams.first().cloned() else {
        return Err("Signed in, but this Apple ID has no developer team.\nWhat to do: sign in once at developer.apple.com/account to set up the free team, then try again.".into());
    };
    ui.log(
        Kind::Good,
        format!(
            "Developer team: {} ({}){}",
            team.name.clone().unwrap_or_else(|| "personal team".into()),
            team.team_id,
            team.r#type.as_ref().map(|t| format!(" · {t}")).unwrap_or_default()
        ),
    );
    ui.step(STEP_CERTS, 1, STEP_CERTS_SIZE);
    match dev.list_ios_certs(&team).await {
        Ok(certs) => {
            ui.log(Kind::Info, format!("Signing certificates on this Apple ID: {}", certs.len()));
            for c in &certs {
                ui.log(
                    Kind::Info,
                    format!(
                        "  • {} — made on \"{}\" — expires {}",
                        c.name.clone().unwrap_or_default(),
                        c.machine_name.clone().unwrap_or_else(|| "unknown".into()),
                        c.expiration_date
                            .as_ref()
                            .map(|d| d.to_xml_format().chars().take(10).collect::<String>())
                            .unwrap_or_default()
                    ),
                );
            }
            if certs.len() >= 2 {
                ui.log(
                    Kind::Warn,
                    "That may be the limit for a free Apple ID: the next install will ask which certificate to revoke.",
                );
            }
        }
        Err(e) => ui.log(Kind::Warn, format!("Couldn't list certificates: {}", clean(e))),
    }
    ui.step(STEP_CERTS, STEP_CERTS_SIZE, STEP_CERTS_SIZE);
    ui.step(STEP_APP_IDS, 1, STEP_APP_IDS_SIZE);
    match dev.list_app_ids(&team, None).await {
        Ok(r) => {
            let used = r.app_ids.len();
            match r.available_quantity {
                Some(left) => ui.log(
                    Kind::Info,
                    format!("App IDs: {used} in use, {left} more allowed right now (free Apple IDs: 10 per 7 days; each app extension needs one)."),
                ),
                None => ui.log(Kind::Info, format!("App IDs in use: {used}.")),
            }
            for a in r.app_ids.iter().take(12) {
                ui.log(Kind::Info, format!("  • {} ({})", a.name, a.identifier));
            }
        }
        Err(e) => ui.log(Kind::Warn, format!("Couldn't list App IDs: {}", clean(e))),
    }
    ui.step(STEP_APP_IDS, STEP_APP_IDS_SIZE, STEP_APP_IDS_SIZE);
    Ok("This Apple ID works: signed in and the developer account is ready for sideloading.".into())
}

/// iOS 16+ only opens sideloaded apps with Developer Mode on. Check it, and if
/// it's off make the switch appear in Settings and say how to turn it on.
async fn check_developer_mode(provider: &idevice::provider::UsbmuxdProvider, ui: &UiLink) {
    use idevice::IdeviceService;
    match idevice::amfi::AmfiClient::connect(provider).await {
        Ok(mut amfi) => match amfi.get_developer_mode_status().await {
            Ok(true) => ui.log(Kind::Good, "Developer Mode is on."),
            Ok(false) => {
                ui.log(Kind::Warn, "Developer Mode is OFF — iOS won't open the app until it's on.");
                if amfi.reveal_developer_mode_option_in_ui().await.is_ok() {
                    ui.log(Kind::Info, "The Developer Mode switch is now shown in the phone's Settings.");
                }
                ui.log(
                    Kind::Info,
                    "Turn it on: Settings › Privacy & Security › Developer Mode › On, restart the phone, then tap \"Turn On\".",
                );
            }
            Err(e) => crate::write_log_line("DBG ", &format!("Developer Mode check failed: {e}")),
        },
        Err(e) => crate::write_log_line("DBG ", &format!("Developer Mode check unavailable: {e}")),
    }
}

fn signed_output_path(original: &Path) -> PathBuf {
    let stem = original.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "app".into());
    let dir = original.parent().map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir);
    let candidate = dir.join(format!("{stem}-signed.ipa"));
    if File::create(&candidate).is_ok() {
        candidate
    } else {
        std::env::temp_dir().join(format!("{stem}-signed.ipa"))
    }
}

// ---------- app file editing ----------

/// Writes a copy of the .ipa with the chosen changes in the Info.plist files.
fn prepare_ipa(src_path: &Path, t: &Tweaks) -> Result<PathBuf, String> {
    let file = File::open(src_path).map_err(|e| e.to_string())?;
    let mut src = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    // find the main app folder, e.g. "Payload/Foo.app/"
    let mut app_prefix = None;
    for i in 0..src.len() {
        let n = src.name_for_index(i).unwrap_or("").replace('\\', "/");
        let parts: Vec<&str> = n.split('/').collect();
        if parts.len() == 3 && parts[0] == "Payload" && parts[1].ends_with(".app") && parts[2] == "Info.plist" {
            app_prefix = Some(format!("Payload/{}/", parts[1]));
            break;
        }
    }
    let app_prefix = app_prefix.ok_or("no app found inside")?;
    let main_plist = format!("{app_prefix}Info.plist");

    // old bundle ID, needed to rename extension IDs
    let mut old_id = String::new();
    {
        let idx = (0..src.len())
            .find(|&i| src.name_for_index(i).map(|n| n.replace('\\', "/")) == Some(main_plist.clone()))
            .ok_or("Info.plist missing")?;
        let mut buf = Vec::new();
        src.by_index(idx).map_err(|e| e.to_string())?.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        if let Ok(v) = plist::Value::from_reader(std::io::Cursor::new(buf)) {
            if let Some(s) = v.as_dictionary().and_then(|d| d.get("CFBundleIdentifier")).and_then(|v| v.as_string()) {
                old_id = s.to_string();
            }
        }
    }
    let new_id = t.bundle_id.trim().to_string();

    let dir = std::env::temp_dir().join("ipabazooka");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out_path = dir.join(format!(
        "{}-prepared.ipa",
        src_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "app".into())
    ));
    let mut out = zip::ZipWriter::new(File::create(&out_path).map_err(|e| e.to_string())?);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);

    let ext_dirs = [format!("{app_prefix}PlugIns/"), format!("{app_prefix}Extensions/")];
    for i in 0..src.len() {
        let raw = src.name_for_index(i).unwrap_or("").to_string();
        let name = raw.replace('\\', "/");
        if t.remove_extensions && ext_dirs.iter().any(|d| name.starts_with(d.as_str())) {
            continue;
        }
        let is_main = name == main_plist;
        let is_ext_plist = name.starts_with(&app_prefix)
            && name.ends_with(".appex/Info.plist")
            && name[app_prefix.len()..].matches('/').count() == 2;
        if is_main || (is_ext_plist && !new_id.is_empty() && !old_id.is_empty()) {
            let mut buf = Vec::new();
            src.by_index(i).map_err(|e| e.to_string())?.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            let mut v = plist::Value::from_reader(std::io::Cursor::new(buf)).map_err(|e| e.to_string())?;
            if let Some(d) = v.as_dictionary_mut() {
                if is_main {
                    if !t.name.trim().is_empty() {
                        d.insert("CFBundleDisplayName".into(), t.name.trim().into());
                    }
                    if !new_id.is_empty() {
                        d.insert("CFBundleIdentifier".into(), new_id.clone().into());
                    }
                    if t.remove_device_limits {
                        d.remove("UISupportedDevices");
                    }
                    if t.file_sharing {
                        d.insert("UIFileSharingEnabled".into(), true.into());
                        d.insert("LSSupportsOpeningDocumentsInPlace".into(), true.into());
                    }
                } else if let Some(id) = d.get("CFBundleIdentifier").and_then(|v| v.as_string()).map(|s| s.to_string()) {
                    if let Some(rest) = id.strip_prefix(&old_id) {
                        d.insert("CFBundleIdentifier".into(), format!("{new_id}{rest}").into());
                    }
                }
            }
            let mut xml = Vec::new();
            plist::to_writer_xml(&mut xml, &v).map_err(|e| e.to_string())?;
            out.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            out.write_all(&xml).map_err(|e| e.to_string())?;
        } else if name == raw {
            let f = src.by_index_raw(i).map_err(|e| e.to_string())?;
            out.raw_copy_file(f).map_err(|e| e.to_string())?;
        } else if name.ends_with('/') {
            out.add_directory(name.as_str(), opts).map_err(|e| e.to_string())?;
        } else {
            let mut buf = Vec::new();
            src.by_index(i).map_err(|e| e.to_string())?.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            out.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            out.write_all(&buf).map_err(|e| e.to_string())?;
        }
    }
    out.finish().map_err(|e| e.to_string())?;
    Ok(out_path)
}

/// Packs a signed .app folder into Payload/<Name>.app inside a new .ipa.
fn zip_app(app_dir: &Path, out_path: &Path) -> Result<(), String> {
    let app_name = app_dir.file_name().ok_or("bad app folder")?.to_string_lossy().to_string();
    let mut out = zip::ZipWriter::new(File::create(out_path).map_err(|e| e.to_string())?);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true)
        .unix_permissions(0o755);
    out.add_directory("Payload/", opts).map_err(|e| e.to_string())?;
    let mut stack = vec![app_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rel = dir.strip_prefix(app_dir).unwrap_or(Path::new(""));
        let rel_s = rel.to_string_lossy().replace('\\', "/");
        let base = if rel_s.is_empty() {
            format!("Payload/{app_name}/")
        } else {
            format!("Payload/{app_name}/{rel_s}/")
        };
        out.add_directory(base.as_str(), opts).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let fname = entry.file_name().to_string_lossy().to_string();
                out.start_file(format!("{base}{fname}"), opts).map_err(|e| e.to_string())?;
                let mut f = File::open(&path).map_err(|e| e.to_string())?;
                std::io::copy(&mut f, &mut out).map_err(|e| e.to_string())?;
            }
        }
    }
    out.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plist_xml(id: &str, name: &str) -> Vec<u8> {
        let mut d = plist::Dictionary::new();
        d.insert("CFBundleIdentifier".into(), id.into());
        d.insert("CFBundleDisplayName".into(), name.into());
        d.insert("UISupportedDevices".into(), plist::Value::Array(vec!["iPhone10,1".into()]));
        let mut out = Vec::new();
        plist::to_writer_xml(&mut out, &plist::Value::Dictionary(d)).unwrap();
        out
    }

    #[test]
    fn sign_in_errors_are_sorted() {
        assert_eq!(
            classify("Failed to log in\nAuth error -22406: Your Apple ID or password was entered incorrectly."),
            SignInProblem::Rejected
        );
        assert_eq!(classify("Failed to provision\nwebsocket closed"), SignInProblem::Server);
        assert_eq!(classify("Auth error -20209: This Apple ID has been locked"), SignInProblem::Other);
        assert!(explain("Auth error -22406: wrong").to_lowercase().contains("security keys"));
        assert_eq!(server_folder("https://ani.sidestore.io"), "anisette-ani.sidestore.io");
        assert_eq!(crate::normalize_apple_id(" Me.Name@iCloud.com\u{200B} "), "Me.Name@iCloud.com");
    }

    /// The failure from the person's log (2026-11-26): Apple's server didn't
    /// answer. It must be retried and explained as a connection problem.
    #[test]
    fn unreachable_apple_is_a_connection_problem() {
        let real = " ● Failed to fetch URL Bag\n ├ vendor\\isideload\\src\\auth\\grandslam.rs:60\n │\n \
                    ● error sending request for url (https://gsa.apple.com/grandslam/GsService2/lookup)";
        assert_eq!(classify(real), SignInProblem::Network);
        let msg = explain(real);
        assert!(msg.contains("not your Apple ID"), "{msg}");
        assert!(msg.contains("VPN"), "{msg}");
        // no arrow (the window's font can't draw it)
        assert!(!msg.contains('→'));
        assert!(msg.contains("Failed to fetch URL Bag › error sending request"), "{msg}");
        // a sign-in helper server being down is still "try the next source"
        assert_eq!(
            classify("Failed to get anisette › error sending request for url (https://ani.example.net/v3/client_info)"),
            SignInProblem::Server
        );
        // and a wrong password stays a wrong password
        assert_eq!(classify("Auth error -22406: incorrect"), SignInProblem::Rejected);
        assert!(advice("error sending request for url (https://developerservices2.apple.com/services/QH65B2/listTeams.action)").is_some());
    }

    #[test]
    fn errors_from_the_real_log_are_explained() {
        // exactly what the library reported in the person's log
        let cert = " ● Failed to retrieve certificate identity\n ├ vendor\\isideload\\src\\sideload\\sideloader.rs:100\n │\n ● Developer error 7460: Maximum number of certificates reached\n ╰ vendor\\isideload\\src\\sideload\\cert_identity.rs:410";
        assert_eq!(
            clean(cert),
            "Failed to retrieve certificate identity › Developer error 7460: Maximum number of certificates reached"
        );
        assert!(advice(cert).unwrap().contains("revoke"));
        let f = failure("Install failed", cert);
        assert!(f.starts_with("Install failed: Failed to retrieve certificate identity"));
        assert!(f.contains("\nWhat to do: "));

        let pw = " ● Failed to log in to Apple ID\n ├ vendor\\isideload\\src\\auth\\apple_account.rs:206\n │\n ● Auth error -22406: Enter the correct password for this Apple Account.\n ╰ vendor\\isideload\\src\\auth\\grandslam.rs:282";
        assert!(!clean(pw).contains(".rs:"));
        assert!(clean(pw).contains("-22406"));
        assert!(advice("Developer error 9401: maximum App ID limit").unwrap().contains("10 App IDs"));
        assert!(advice("all good").is_none());
    }

    #[test]
    fn emails_are_masked_like_the_library() {
        assert_eq!(mask_email("jane.doe@example.com"), "j***e@example.com");
        assert_eq!(mask_email("lee.smith02@example.com"), "l***2@example.com");
        assert_eq!(mask_email("nope"), "***");
        assert!(explain("Auth error -22406: Enter the correct password").contains("app-specific password"));
    }

    #[test]
    fn file_storage_round_trip() {
        use isideload::util::storage::SideloadingStorage;
        let s = FileStorage::new("test-storage");
        s.store("k:1", "hello").unwrap();
        assert_eq!(s.retrieve("k:1").unwrap().as_deref(), Some("hello"));
        s.delete("k:1").unwrap();
        assert_eq!(s.retrieve("k:1").unwrap(), None);
        let _ = std::fs::remove_dir(data_dir().join("test-storage"));
    }

    #[test]
    fn tweaks_are_applied() {
        let dir = std::env::temp_dir().join("ipabazooka-test");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("t.ipa");
        {
            let mut z = zip::ZipWriter::new(File::create(&src).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("Payload/T.app/Info.plist", o).unwrap();
            z.write_all(&plist_xml("com.old.app", "Old")).unwrap();
            z.start_file("Payload/T.app/T", o).unwrap();
            z.write_all(b"binary").unwrap();
            z.start_file("Payload/T.app/PlugIns/W.appex/Info.plist", o).unwrap();
            z.write_all(&plist_xml("com.old.app.widget", "W")).unwrap();
            z.finish().unwrap();
        }
        let t = Tweaks {
            name: "New".into(),
            bundle_id: "com.new.app".into(),
            remove_device_limits: true,
            file_sharing: true,
            remove_extensions: false,
        };
        let out = prepare_ipa(&src, &t).unwrap();
        let info = crate::ipa::read_ipa(&out).unwrap();
        assert_eq!(info.name, "New");
        assert_eq!(info.bundle_id, "com.new.app");
        assert_eq!(info.extensions, 1);
        let mut z = zip::ZipArchive::new(File::open(&out).unwrap()).unwrap();
        let mut buf = Vec::new();
        z.by_name("Payload/T.app/PlugIns/W.appex/Info.plist").unwrap().read_to_end(&mut buf).unwrap();
        assert!(String::from_utf8_lossy(&buf).contains("com.new.app.widget"));
        buf.clear();
        z.by_name("Payload/T.app/Info.plist").unwrap().read_to_end(&mut buf).unwrap();
        let s = String::from_utf8_lossy(&buf);
        assert!(!s.contains("UISupportedDevices") && s.contains("UIFileSharingEnabled"));

        let t2 = Tweaks { remove_extensions: true, ..Default::default() };
        let out2 = prepare_ipa(&src, &t2).unwrap();
        assert_eq!(crate::ipa::read_ipa(&out2).unwrap().extensions, 0);

        // pack a folder back into an ipa
        let app_dir = dir.join("Z.app");
        std::fs::create_dir_all(app_dir.join("sub")).unwrap();
        std::fs::write(app_dir.join("Info.plist"), plist_xml("com.z", "Z")).unwrap();
        std::fs::write(app_dir.join("sub").join("f.txt"), b"x").unwrap();
        let zipped = dir.join("z.ipa");
        zip_app(&app_dir, &zipped).unwrap();
        assert_eq!(crate::ipa::read_ipa(&zipped).unwrap().bundle_id, "com.z");
    }
}
