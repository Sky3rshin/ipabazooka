#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod device;
mod ipa;
mod local_anisette;
mod sideload;
mod phone;
mod progress;
mod theme;

use theme::{lbl, th};

use device::Device;
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, RichText, Sense, Stroke, StrokeKind,
    Vec2,
};
use ipa::IpaInfo;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};


#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Info,
    Good,
    Warn,
    Bad,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Install,
    Export,
}

struct Run {
    events: Receiver<sideload::Event>,
    answers: sideload::Answers,
}

struct LoaderApp {
    // devices
    devices: Vec<Device>,
    selected_udid: Option<String>,
    scan_rx: Option<Receiver<Result<Vec<Device>, String>>>,
    last_scan: f64,
    device_note: String,
    service_ok: Option<bool>,
    // inputs
    apple_id: String,
    ipa: Option<IpaInfo>,
    // advanced options
    mode: Mode,
    custom_name: String,
    custom_bundle: String,
    remove_device_limits: bool,
    file_sharing: bool,
    remove_extensions: bool,
    revoke_certs: bool,
    /// 0 = automatic, 1..=n = one server from the list, n+1 = custom
    anisette_choice: usize,
    anisette_custom: String,
    /// use this PC's iCloud for Windows for sign-in data
    local_mode: sideload::LocalAnisette,
    show_password: bool,
    /// the current/next run only checks the Apple ID
    checking: bool,
    share_extension_ids: bool,
    more_memory: bool,
    cert_req: Option<Vec<sideload::CertInfo>>,
    cert_pick: Vec<bool>,
    failure: Option<String>,
    show_phone: bool,
    phone_tx: Option<std::sync::mpsc::Sender<phone::PhoneEvent>>,
    phone_rx: Option<Receiver<phone::PhoneEvent>>,
    phone_busy: u32,
    phone_apps: Option<Vec<phone::PhoneApp>>,
    phone_dev_mode: Option<bool>,
    phone_msg: String,
    confirm_remove: Option<String>,
    /// scans in a row a device has been missing (to ignore brief USB blips)
    missing: std::collections::HashMap<String, u32>,
    // running
    asking_password: bool,
    password: String,
    run: Option<Run>,
    code_req: Option<sideload::CodeRequest>,
    code_input: String,
    /// the newest log line (or the running steps), cut like the original's status label
    status: String,
    /// Some(true) after a job finished well, Some(false) after it failed
    last_result: Option<bool>,
    tracker: progress::Tracker,
    bar: progress::View,
    /// the log box under the status row (the original hides it until Alt+.)
    show_history: bool,
    /// the Advanced options panel (Alt+V)
    show_advanced: bool,
    /// the Help drop-down was opened at least once (until then its button is highlighted)
    help_seen: bool,
    /// the height last asked for while fitting the window to its content
    last_fit_h: f32,
    /// the window height before the Help drop-down made it taller (put back on close)
    help_restore_h: Option<f32>,
    log: Vec<(Kind, String)>,
    // screenshot mode
    shot_path: Option<PathBuf>,
    frames: u32,
    shot_sent_recently: bool,
    shot_sent_frame: u32,
}

impl LoaderApp {
    fn new(shot_path: Option<PathBuf>) -> Self {
        let mut app = LoaderApp {
            devices: Vec::new(),
            selected_udid: None,
            scan_rx: None,
            last_scan: -100.0,
            device_note: "Looking for devices…".into(),
            service_ok: None,
            apple_id: String::new(),
            ipa: None,
            mode: Mode::Install,
            custom_name: String::new(),
            custom_bundle: String::new(),
            remove_device_limits: false,
            file_sharing: false,
            remove_extensions: false,
            revoke_certs: false,
            anisette_choice: 0,
            anisette_custom: String::new(),
            local_mode: sideload::LocalAnisette::Auto,
            show_password: false,
            checking: false,
            share_extension_ids: false,
            more_memory: false,
            cert_req: None,
            cert_pick: Vec::new(),
            failure: None,
            show_phone: false,
            phone_tx: None,
            phone_rx: None,
            phone_busy: 0,
            phone_apps: None,
            phone_dev_mode: None,
            phone_msg: String::new(),
            confirm_remove: None,
            missing: std::collections::HashMap::new(),
            asking_password: false,
            password: String::new(),
            run: None,
            code_req: None,
            code_input: String::new(),
            status: "Idle.".into(),
            last_result: None,
            tracker: progress::Tracker::new(),
            bar: progress::Tracker::new().view(),
            show_history: option_load("show-history").map(|v| v.trim() == "1").unwrap_or(false),
            show_advanced: false,
            help_seen: option_load("help-seen").is_some(),
            last_fit_h: 0.0,
            help_restore_h: None,
            log: Vec::new(),
            shot_path,
            frames: 0,
            shot_sent_recently: false,
            shot_sent_frame: 0,
        };
        app.push(Kind::Info, "Ready. Drop an .ipa file on the box or click it to choose one.");
        match local_anisette::unavailable_reason() {
            None => app.push(Kind::Good, "Sign-in data: this PC's iCloud for Windows is ready (used first)."),
            Some(why) => app.push(
                Kind::Warn,
                format!("Sign-in data: this PC's iCloud can't be used — {why}. Online servers will be used instead."),
            ),
        }
        app
    }

    fn push(&mut self, kind: Kind, text: impl Into<String>) {
        let text = text.into();
        if self.shot_path.is_none() {
            write_log_file(kind, &text);
        }
        // like the original, the status row always shows the newest line
        self.status = progress::status_line(&text);
        self.log.push((kind, text));
        if self.log.len() > 500 {
            self.log.remove(0);
        }
    }

    /// A step's line: swap its earlier line in place (the last whole line with
    /// that text), or add it when there is none.
    fn apply_log_change(&mut self, change: progress::LogChange) {
        if let Some(old) = &change.replaces {
            if let Some(entry) = self.log.iter_mut().rev().find(|(_, t)| t == old) {
                if self.shot_path.is_none() {
                    write_log_file(Kind::Info, &change.line);
                }
                entry.1 = change.line.clone();
                self.status = progress::status_line(&change.line);
                return;
            }
        }
        self.push(Kind::Info, change.line);
    }

    /// Why Start can't be pressed right now (None = it can). The original
    /// greys Start out until a file and (unless exporting) a device are chosen,
    /// and while a job runs.
    fn start_blocker(&self) -> Option<&'static str> {
        if self.run.is_some() {
            Some("Working…")
        } else if self.ipa.is_none() {
            Some("Choose an .ipa file first (drop it on the window or click the box).")
        } else if self.mode == Mode::Install && self.selected_device().is_none() {
            Some("Plug in your iPhone or iPad with a cable, unlock it and tap \"Trust\".")
        } else {
            None
        }
    }

    fn set_history(&mut self, ctx: &egui::Context, show: bool) {
        if show == self.show_history {
            return;
        }
        self.show_history = show;
        option_save("show-history", if show { "1" } else { "0" });
        let (full, maxed, size, monitor) = ctx.input(|i| {
            let vp = i.viewport();
            (
                vp.fullscreen.unwrap_or(false),
                vp.maximized.unwrap_or(false),
                vp.inner_rect.map(|r| r.size()),
                vp.monitor_size,
            )
        });
        if show && !full && !maxed {
            // make room for the history under the status row
            if let Some(size) = size {
                let cap = monitor.map(|m| m.y - 60.0).unwrap_or(900.0);
                let h = (size.y + HISTORY_ROOM).min(cap).max(size.y);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(size.x, h)));
            }
        }
        // the next hide fits the window to its content again
        self.last_fit_h = 0.0;
    }

    fn selected_device(&self) -> Option<Device> {
        self.selected_udid
            .as_ref()
            .and_then(|u| self.devices.iter().find(|d| &d.udid == u).cloned())
            .or_else(|| self.devices.first().cloned())
    }

    fn start_scan(&mut self, now: f64) {
        if self.scan_rx.is_some() {
            return;
        }
        self.last_scan = now;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(device::list_devices());
        });
        self.scan_rx = Some(rx);
    }

    fn poll_scan(&mut self) {
        let Some(rx) = &self.scan_rx else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.scan_rx = None;
        match result {
            Ok(mut found) => {
                let first_time = self.service_ok != Some(true);
                self.service_ok = Some(true);
                for d in &found {
                    self.missing.remove(&d.udid);
                    if !self.devices.iter().any(|o| o.udid == d.udid) {
                        self.push(Kind::Good, format!("Device connected: {}", d.label()));
                    }
                }
                // A phone that vanishes for one scan is usually a USB blip (or
                // the phone busy installing); only call it gone after two misses.
                // While a job runs, keep it in the list regardless.
                for o in self.devices.clone() {
                    if !found.iter().any(|d| d.udid == o.udid) {
                        let n = self.missing.entry(o.udid.clone()).or_insert(0);
                        *n += 1;
                        if *n < 2 || self.run.is_some() {
                            found.push(o);
                        } else {
                            self.missing.remove(&o.udid);
                            self.push(Kind::Warn, format!("Device disconnected: {}", o.name));
                        }
                    }
                }
                if found.is_empty() && (first_time || !self.devices.is_empty()) {
                    self.push(Kind::Info, "No iPhone or iPad plugged in. Connect one with a cable and tap \"Trust\".");
                }
                self.device_note = match found.len() {
                    0 => "No device plugged in".into(),
                    1 => "1 device found".into(),
                    n => format!("{n} devices found"),
                };
                let had_selection = self
                    .selected_udid
                    .as_ref()
                    .map(|u| found.iter().any(|d| &d.udid == u))
                    .unwrap_or(false);
                self.devices = found;
                if !had_selection && self.run.is_none() {
                    self.selected_udid = self.devices.first().map(|d| d.udid.clone());
                }
            }
            Err(e) => {
                if self.service_ok != Some(false) {
                    self.push(Kind::Warn, format!("{e}. It's needed to talk to an iPhone or iPad."));
                }
                self.service_ok = Some(false);
                self.devices.clear();
                self.device_note = "Apple device service not found".into();
            }
        }
    }

    fn load_ipa(&mut self, path: PathBuf) {
        if self.run.is_some() {
            return;
        }
        match ipa::read_ipa(&path) {
            Ok(info) => {
                self.push(
                    Kind::Good,
                    format!(
                        "Loaded {} — {} {} ({}), {}",
                        info.file_name,
                        info.name,
                        info.version,
                        info.bundle_id,
                        ipa::human_size(info.size_bytes)
                    ),
                );
                self.bar = progress::Tracker::new().view();
                self.last_result = None;
                self.ipa = Some(info);
            }
            Err(e) => {
                self.push(Kind::Bad, format!("{}: {e}", path.display()));
            }
        }
    }

    fn pick_file(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .set_title("Choose an iOS app (.ipa)")
            .add_filter("iOS app", &["ipa"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.load_ipa(p);
        }
    }

    /// `check` = only test the Apple ID (no app or phone needed).
    fn try_start(&mut self, check: bool) {
        self.checking = check;
        if !check && self.ipa.is_none() {
            self.push(Kind::Bad, "Choose an .ipa file first.");
            return;
        }
        let id = self.apple_id.trim();
        if !(id.contains('@') && id.contains('.')) {
            self.push(Kind::Bad, "Enter your Apple ID email address.");
            return;
        }
        if check {
            self.password.clear();
            self.asking_password = true;
            return;
        }
        if !self.custom_bundle.trim().is_empty() && !self.custom_bundle.contains('.') {
            self.push(Kind::Bad, "A custom bundle ID looks like com.example.myapp.");
            return;
        }
        if self.mode == Mode::Install && self.selected_device().is_none() {
            self.push(Kind::Bad, "Plug in your iPhone or iPad with a cable, unlock it and tap \"Trust\".");
            return;
        }
        self.password.clear();
        self.asking_password = true;
    }

    /// Sign-in servers to use, from the Advanced options choice.
    fn anisette_servers(&self) -> Vec<String> {
        let list = sideload::ANISETTE_SERVERS;
        match self.anisette_choice {
            0 => list.iter().map(|s| s.to_string()).collect(),
            n if n <= list.len() => vec![list[n - 1].to_string()],
            _ => {
                let c = self.anisette_custom.trim().trim_end_matches('/').to_string();
                if c.starts_with("https://") || c.starts_with("http://") {
                    vec![c]
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn begin_run(&mut self, ctx: &egui::Context) {
        let check = self.checking;
        let info = self.ipa.clone();
        if !check && info.is_none() {
            return;
        }
        let installing = !check && self.mode == Mode::Install;
        let udid = if installing {
            match self.selected_device() {
                Some(d) => Some(d.udid),
                None => {
                    self.push(Kind::Bad, "Plug in your iPhone or iPad first.");
                    return;
                }
            }
        } else {
            None
        };
        if self.password != self.password.trim() {
            self.push(
                Kind::Warn,
                "Your password starts or ends with a space. If it was pasted, check that no extra space came along.",
            );
        }
        if self.anisette_choice == sideload::ANISETTE_SERVERS.len() + 1 && self.anisette_servers().is_empty() {
            self.push(Kind::Bad, "Enter the custom sign-in server address (starting with https://) or choose Automatic.");
            return;
        }
        let job = sideload::Job {
            email: normalize_apple_id(&self.apple_id),
            anisette_servers: self.anisette_servers(),
            local_anisette: self.local_mode,
            password: std::mem::take(&mut self.password),
            ipa: info.as_ref().map(|i| i.path.clone()).unwrap_or_default(),
            udid,
            export_only: !check && self.mode == Mode::Export,
            revoke_certs: self.revoke_certs,
            share_extension_ids: self.share_extension_ids,
            more_memory: self.more_memory,
            check_only: check,
            tweaks: if check {
                sideload::Tweaks::default()
            } else {
                sideload::Tweaks {
                    name: self.custom_name.clone(),
                    bundle_id: self.custom_bundle.clone(),
                    remove_device_limits: self.remove_device_limits,
                    file_sharing: self.file_sharing,
                    remove_extensions: self.remove_extensions,
                }
            },
        };
        // a clear record of what was attempted, for the saved log
        let what = if check {
            "Apple ID check"
        } else if installing {
            "install"
        } else {
            "export signed .ipa"
        };
        self.push(
            Kind::Info,
            format!("──── ipabazooka {} · {what} · theme {} ────", env!("CARGO_PKG_VERSION"), th().name),
        );
        self.push(Kind::Info, format!("Apple ID: {}", sideload::mask_email(&normalize_apple_id(&self.apple_id))));
        if installing {
            if let Some(d) = self.selected_device() {
                self.push(
                    Kind::Info,
                    format!("Device: {} · model {} · iOS {} · {} · UDID {}", d.name, d.model, d.ios, d.connection, d.udid),
                );
            }
        }
        if let (false, Some(info)) = (check, &info) {
            self.push(
                Kind::Info,
                format!(
                    "App: {} {} ({}) · {} · {} extension(s) · needs iOS {} · file {}",
                    info.name,
                    info.version,
                    info.bundle_id,
                    ipa::human_size(info.size_bytes),
                    info.extensions,
                    if info.min_ios.is_empty() { "?" } else { &info.min_ios },
                    info.file_name
                ),
            );
        }
        let (tx, rx) = channel();
        let answers = sideload::spawn(job, sideload::UiLink::new(tx, ctx.clone()));
        self.tracker = progress::Tracker::new();
        self.bar = self.tracker.view();
        self.last_result = None;
        self.failure = None;
        self.run = Some(Run { events: rx, answers });
        match (check, &info) {
            (true, _) => self.push(Kind::Info, "Checking the Apple ID (no app or phone needed)…"),
            (false, Some(info)) => self.push(
                Kind::Info,
                format!("Starting {} of {}…", if installing { "install" } else { "signing" }, info.name),
            ),
            _ => {}
        }
    }

    fn tick_run(&mut self, _now: f64) {
        let mut events = Vec::new();
        if let Some(run) = &self.run {
            while let Ok(e) = run.events.try_recv() {
                events.push(e);
            }
        }
        for e in events {
            match e {
                sideload::Event::Log(k, s) => self.push(k, s),
                sideload::Event::Progress(msg, done, total) => {
                    if let Some(change) = self.tracker.update(&msg, done, total) {
                        self.apply_log_change(change);
                    }
                    self.bar = self.tracker.view();
                    // the running steps and the latest detail replace the status text
                    let label = self.bar.label();
                    if !label.is_empty() {
                        self.status = progress::status_line(&label);
                    }
                }
                sideload::Event::NeedCode(req) => {
                    if let Some(err) = &req.last_error {
                        self.push(Kind::Warn, format!("Code problem: {err}"));
                    }
                    self.push(Kind::Info, "Two-factor code needed.");
                    self.code_input.clear();
                    self.code_req = Some(req);
                }
                sideload::Event::NeedCertChoice(certs) => {
                    // pre-tick certificates not made by this app (usually old Sideloadly/AltStore ones)
                    self.cert_pick = certs.iter().map(|c| c.machine != "ipabazooka").collect();
                    if !self.cert_pick.iter().any(|b| *b) {
                        if let Some(first) = self.cert_pick.first_mut() {
                            *first = true;
                        }
                    }
                    self.cert_req = Some(certs);
                }
                sideload::Event::Finished(result) => {
                    self.run = None;
                    self.code_req = None;
                    self.cert_req = None;
                    match result {
                        Ok(msg) => {
                            self.tracker.finish();
                            self.bar = self.tracker.view();
                            self.last_result = Some(true);
                            self.push(Kind::Good, msg);
                            if self.mode == Mode::Install && !self.checking {
                                self.push(
                                    Kind::Info,
                                    "With a free Apple ID the app works for 7 days; install it again to renew.",
                                );
                            }
                        }
                        Err(err) => {
                            self.last_result = Some(false);
                            // full story in a pop-up and the log; the status row
                            // keeps the first (main) reason
                            let first = err.lines().next().unwrap_or("Failed").to_string();
                            for line in err.lines() {
                                self.push(Kind::Bad, line.to_string());
                            }
                            self.status = progress::status_line(&format!("Failed — {first}"));
                            self.failure = Some(err);
                        }
                    }
                }
            }
        }
    }

    fn answer_code(&mut self, answer: sideload::CodeAnswer) {
        if let Some(run) = &self.run {
            let _ = run.answers.code_tx.send(answer);
        }
        self.code_req = None;
        self.code_input.clear();
    }

    /// "Which certificate should be revoked?" — shown when the Apple ID is at its limit.
    fn cert_window(&mut self, ctx: &egui::Context) {
        let Some(certs) = self.cert_req.clone() else { return };
        let mut answer: Option<sideload::CertAnswer> = None;
        egui::Window::new("Certificate limit reached")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([440.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Your Apple ID already has the most signing certificates Apple allows. Pick the one(s) to revoke so a new one can be made:");
                ui.add_space(6.0);
                for (i, c) in certs.iter().enumerate() {
                    let made_by = if c.machine.is_empty() { "unknown computer".to_string() } else { c.machine.clone() };
                    let label = format!("{} — made on \"{}\" — expires {}", c.name, made_by, c.expires);
                    if let Some(v) = self.cert_pick.get_mut(i) {
                        ui.checkbox(v, label);
                    }
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Apps already installed with a revoked certificate stop opening until you reinstall them.")
                        .size(11.0)
                        .color(th().warn),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let any = self.cert_pick.iter().any(|b| *b);
                    let ok = egui::Button::new(RichText::new("Revoke selected").color(th().ink)).fill(th().accent);
                    if ui.add_enabled(any, ok).clicked() {
                        let picked: Vec<String> = certs
                            .iter()
                            .zip(self.cert_pick.iter())
                            .filter(|(_, p)| **p)
                            .map(|(c, _)| c.serial.clone())
                            .collect();
                        answer = Some(Some(picked));
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(None);
                    }
                });
            });
        if let Some(a) = answer {
            if let Some(run) = &self.run {
                let _ = run.answers.cert_tx.send(a);
            }
            self.cert_req = None;
        }
    }

    /// Pop-up after a failure: what went wrong and what to do.
    fn failure_window(&mut self, ctx: &egui::Context) {
        let Some(msg) = self.failure.clone() else { return };
        let mut close = false;
        egui::Window::new(lbl("[ ERROR ]", "Something went wrong"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([460.0, 0.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for (i, line) in msg.lines().enumerate() {
                        let rich = RichText::new(line).size(if i == 0 { 13.0 } else { 12.0 });
                        if i == 0 {
                            ui.label(rich.color(th().bad).strong());
                        } else if line.starts_with("What to do") {
                            ui.label(rich.color(th().text).strong());
                        } else {
                            ui.label(rich.color(th().text));
                        }
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        close = true;
                    }
                    if ui.button("Open saved log").clicked() {
                        open_log_file();
                    }
                    if ui.button("Copy log").clicked() {
                        ctx.copy_text(self.log_text());
                    }
                });
            });
        if close {
            self.failure = None;
        }
    }

    fn log_text(&self) -> String {
        self.log.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n")
    }

    /// Phone tools: apps on the phone, remove one, Developer Mode.
    fn phone_window(&mut self, ctx: &egui::Context) {
        // collect results from background jobs
        if let Some(rx) = &self.phone_rx {
            let mut got = Vec::new();
            while let Ok(e) = rx.try_recv() {
                got.push(e);
            }
            for e in got {
                self.phone_busy = self.phone_busy.saturating_sub(1);
                match e {
                    phone::PhoneEvent::Apps(Ok(list)) => {
                        self.phone_msg = format!("{} app(s) installed by you.", list.len());
                        self.phone_apps = Some(list);
                    }
                    phone::PhoneEvent::Apps(Err(e)) => {
                        self.phone_msg = e.clone();
                        self.push(Kind::Bad, e);
                    }
                    phone::PhoneEvent::DevMode(r) => match r {
                        Ok(on) => {
                            self.phone_dev_mode = Some(on);
                            self.push(Kind::Info, format!("Developer Mode is {}.", if on { "on" } else { "off" }));
                        }
                        Err(e) => {
                            self.phone_msg = e.clone();
                            self.push(Kind::Bad, e);
                        }
                    },
                    phone::PhoneEvent::Done(what, r) => match r {
                        Ok(m) => {
                            self.phone_msg = m.clone();
                            self.push(Kind::Good, m);
                            if what.starts_with("Remove") {
                                self.phone_job(ctx, phone::PhoneJob::ListApps);
                            }
                        }
                        Err(e) => {
                            self.phone_msg = e.clone();
                            self.push(Kind::Bad, e);
                        }
                    },
                }
            }
        }
        if !self.show_phone {
            return;
        }
        let Some(dev) = self.selected_device() else {
            self.show_phone = false;
            self.push(Kind::Bad, "Plug in your iPhone or iPad to use Phone tools.");
            return;
        };
        let mut open = true;
        let mut job: Option<phone::PhoneJob> = None;
        egui::Window::new(lbl("[ PHONE_TOOLS ]", "Phone tools"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([460.0, 420.0])
            .show(ctx, |ui| {
                ui.label(RichText::new(&dev.name).strong().size(15.0).color(th().text));
                ui.label(
                    RichText::new(format!("{} · iOS {} · {}", dev.model, dev.ios, dev.connection))
                        .size(11.0)
                        .color(th().muted),
                );
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("UDID {}", dev.udid)).size(11.0).color(th().muted).monospace());
                    if ui.small_button("Copy").clicked() {
                        ctx.copy_text(dev.udid.clone());
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Developer Mode:").strong());
                    let (t, c) = match self.phone_dev_mode {
                        Some(true) => ("On", th().good),
                        Some(false) => ("Off — sideloaded apps won't open", th().bad),
                        None => ("unknown", th().muted),
                    };
                    ui.label(RichText::new(t).color(c));
                });
                ui.horizontal(|ui| {
                    if ui.button("Check").clicked() {
                        job = Some(phone::PhoneJob::DevModeStatus);
                    }
                    if ui
                        .button("Show the switch in Settings")
                        .on_hover_text("Makes Settings › Privacy & Security › Developer Mode appear on the phone")
                        .clicked()
                    {
                        job = Some(phone::PhoneJob::RevealDevMode);
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Apps you installed").strong());
                    if ui.button("Refresh").clicked() {
                        job = Some(phone::PhoneJob::ListApps);
                    }
                    if self.phone_busy > 0 {
                        ui.spinner();
                    }
                });
                if !self.phone_msg.is_empty() {
                    ui.label(RichText::new(&self.phone_msg).size(11.0).color(th().muted));
                }
                if let Some(apps) = self.phone_apps.clone() {
                    egui::ScrollArea::vertical().max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
                        for a in &apps {
                            ui.horizontal(|ui| {
                                let tag = if a.sideloaded { "sideloaded" } else { "App Store" };
                                ui.label(RichText::new(&a.name).strong().color(th().text));
                                ui.label(
                                    RichText::new(format!("{} · v{} · {tag}", a.bundle_id, a.version))
                                        .size(11.0)
                                        .color(th().muted),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if self.confirm_remove.as_deref() == Some(a.bundle_id.as_str()) {
                                        if ui.button(RichText::new("Yes, remove").color(th().bad)).clicked() {
                                            job = Some(phone::PhoneJob::Remove(a.bundle_id.clone()));
                                            self.confirm_remove = None;
                                        }
                                        if ui.small_button("No").clicked() {
                                            self.confirm_remove = None;
                                        }
                                    } else if ui.small_button("Remove").clicked() {
                                        self.confirm_remove = Some(a.bundle_id.clone());
                                    }
                                });
                            });
                        }
                    });
                } else {
                    ui.label(RichText::new("Press Refresh to list the apps on the phone.").size(11.0).color(th().muted));
                }
            });
        if let Some(j) = job {
            self.phone_job(ctx, j);
        }
        if !open {
            self.show_phone = false;
        }
    }

    fn phone_job(&mut self, ctx: &egui::Context, job: phone::PhoneJob) {
        let Some(dev) = self.selected_device() else { return };
        if self.phone_rx.is_none() {
            let (tx, rx) = channel();
            self.phone_tx = Some(tx);
            self.phone_rx = Some(rx);
        }
        if let Some(tx) = &self.phone_tx {
            self.phone_busy += 1;
            self.phone_msg = "Working…".into();
            phone::spawn(dev.udid, job, tx.clone(), ctx.clone());
        }
    }

    fn code_window(&mut self, ctx: &egui::Context) {
        let Some(req) = &self.code_req else { return };
        let sms = req.sms;
        let unknown = req.unknown;
        let numbers = req.numbers.clone();
        let screen = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("dim2")))
            .rect_filled(screen, 0.0, Color32::from_black_alpha(90));
        let mut answer: Option<sideload::CodeAnswer> = None;
        egui::Window::new("Two-factor code")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([340.0, 0.0])
            .show(ctx, |ui| {
                if unknown {
                    ui.label("The last method didn't work. Choose where to get a code:");
                } else if sms {
                    ui.label("Enter the 6-digit code sent by text message.");
                } else {
                    ui.label("Enter the 6-digit code shown on your other Apple devices.");
                }
                ui.add_space(4.0);
                if !unknown {
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.code_input)
                            .hint_text("123456")
                            .font(FontId::monospace(18.0))
                            .desired_width(f32::INFINITY),
                    );
                    r.request_focus();
                    let code: String = self.code_input.chars().filter(|c| c.is_ascii_digit()).collect();
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && code.len() == 6 {
                        answer = Some(sideload::CodeAnswer::SubmitCode(code.clone()));
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let ok = egui::Button::new(RichText::new("Submit").color(th().ink)).fill(th().accent);
                        if ui.add_enabled(code.len() == 6, ok).clicked() {
                            answer = Some(sideload::CodeAnswer::SubmitCode(code.clone()));
                        }
                        if ui.button("Send again").clicked() {
                            answer = Some(sideload::CodeAnswer::ResendCode);
                        }
                        if ui.button("Cancel").clicked() {
                            answer = Some(sideload::CodeAnswer::Abort);
                        }
                    });
                }
                if unknown || !numbers.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Other ways to get a code").size(11.0).color(th().muted));
                    if (unknown || sms) && ui.button("Send to my Apple devices").clicked() {
                        answer = Some(sideload::CodeAnswer::SendToDevices);
                    }
                    for (id, number) in &numbers {
                        if ui.button(format!("Text {number}")).clicked() {
                            answer = Some(sideload::CodeAnswer::SendSms(*id));
                        }
                    }
                    if unknown && ui.button("Cancel").clicked() {
                        answer = Some(sideload::CodeAnswer::Abort);
                    }
                }
            });
        if let Some(a) = answer {
            self.answer_code(a);
        }
    }

    // ---------- UI pieces ----------

    fn drop_tile(&mut self, ui: &mut egui::Ui, hovering_files: bool) {
        let size = Vec2::new(168.0, 168.0);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let hot = resp.hovered() || hovering_files;
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(14), if hovering_files { th().w_hovered } else { th().card });
        p.rect_stroke(
            rect,
            CornerRadius::same(14),
            Stroke::new(if hot { 2.0_f32 } else { 1.0 }, if hot { th().accent } else { th().border }),
            StrokeKind::Inside,
        );
        let c = rect.center();
        match &self.ipa {
            None => {
                let badge = egui::Rect::from_center_size(c - Vec2::new(0.0, 22.0), Vec2::new(64.0, 64.0));
                p.rect_filled(badge, CornerRadius::same(16), th().accent);
                p.rect_filled(
                    egui::Rect::from_min_size(badge.min, Vec2::new(64.0, 30.0)),
                    CornerRadius { nw: 16, ne: 16, sw: 0, se: 0 },
                    theme::lighten(th().accent, 0.35),
                );
                p.text(badge.center(), Align2::CENTER_CENTER, "IPA", theme::font(22.0), th().ink);
                p.text(
                    c + Vec2::new(0.0, 30.0),
                    Align2::CENTER_CENTER,
                    if hovering_files { lbl("RELEASE TO LOAD", "Release to load") } else { lbl("DROP .IPA PAYLOAD", "Drop .ipa here") },
                    theme::font(15.0),
                    th().text,
                );
                p.text(c + Vec2::new(0.0, 50.0), Align2::CENTER_CENTER, "or click to browse", theme::font(12.0), th().muted);
            }
            Some(info) => {
                let badge = egui::Rect::from_center_size(c - Vec2::new(0.0, 26.0), Vec2::new(64.0, 64.0));
                let hue = info.bundle_id.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
                let col = egui::ecolor::HsvaGamma { h: (hue % 360) as f32 / 360.0, s: 0.6, v: 0.8, a: 1.0 };
                let _ = col;
                p.rect_filled(badge, CornerRadius::same(16), th().accent_dark);
                let letter = info.name.chars().next().unwrap_or('?').to_uppercase().to_string();
                p.text(badge.center(), Align2::CENTER_CENTER, letter, theme::font(30.0), th().ink);
                let name = shorten(&info.name, 20);
                p.text(c + Vec2::new(0.0, 24.0), Align2::CENTER_CENTER, name, theme::font(15.0), th().text);
                p.text(
                    c + Vec2::new(0.0, 43.0),
                    Align2::CENTER_CENTER,
                    format!("v{}  ·  {}", info.version, ipa::human_size(info.size_bytes)),
                    theme::font(12.0),
                    th().muted,
                );
                p.text(c + Vec2::new(0.0, 62.0), Align2::CENTER_CENTER, "click to change", theme::font(11.0), th().accent);
            }
        }
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && self.run.is_none() {
            self.pick_file();
        }
    }

    fn right_column(&mut self, ui: &mut egui::Ui, now: f64) {
        let running = self.run.is_some();
        let field_w = ui.available_width();
        ui.label(RichText::new(lbl("> TARGET_DEVICE", "Device")).color(th().muted).size(12.0));
        ui.horizontal(|ui| {
            let all = self.devices.clone();
            let current = self.selected_device();
            let current_udid = current.as_ref().map(|d| d.udid.clone()).unwrap_or_default();
            let shown = current
                .map(|d| d.label())
                .unwrap_or_else(|| "No device — plug in an iPhone or iPad".into());
            ui.add_enabled_ui(!running, |ui| {
                // a fixed slot, so a long name is shortened instead of pushing
                // the two buttons beside it out of the window
                let gap = ui.spacing().item_spacing.x;
                let combo_w = (field_w - 2.0 * (30.0 + gap) - 2.0).max(120.0);
                ui.allocate_ui_with_layout(Vec2::new(combo_w, 22.0), Layout::left_to_right(Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("device")
                        .width(combo_w)
                        .truncate()
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            if all.is_empty() {
                                ui.label(RichText::new("No device found").color(th().muted));
                            }
                            for d in &all {
                                let sel = d.udid == current_udid;
                                if ui.selectable_label(sel, d.label()).clicked() {
                                    self.selected_udid = Some(d.udid.clone());
                                }
                            }
                        });
                });
                let busy = self.scan_rx.is_some();
                if ui
                    .add_sized([30.0, 22.0], egui::Button::new(if busy { "…" } else { "⟳" }))
                    .on_hover_text("Look for devices again")
                    .clicked()
                {
                    self.start_scan(now);
                }
                if ui
                    .add_sized([30.0, 22.0], egui::Button::new("📱"))
                    .on_hover_text("Phone tools: apps on the phone, remove apps, Developer Mode")
                    .clicked()
                {
                    self.show_phone = !self.show_phone;
                    if self.show_phone && self.phone_apps.is_none() {
                        self.phone_job(ui.ctx(), phone::PhoneJob::ListApps);
                        self.phone_job(ui.ctx(), phone::PhoneJob::DevModeStatus);
                    }
                }
            });
        });
        let note_color = match self.service_ok {
            Some(false) => th().warn,
            _ if self.devices.is_empty() => th().muted,
            _ => th().good,
        };
        ui.label(RichText::new(&self.device_note).color(note_color).size(11.0));
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new(lbl("> APPLE_ID", "Apple ID")).color(th().muted).size(12.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let link = egui::Button::new(
                    RichText::new(lbl("[ test login ]", "Check my Apple ID only")).color(th().accent).size(11.0),
                )
                .frame(false);
                if ui
                    .add_enabled(!running, link)
                    .on_hover_text("Signs in and reports on the developer account (certificates, App IDs left). No app or phone needed.")
                    .clicked()
                {
                    self.try_start(true);
                }
            });
        });
        ui.add_enabled(
            !running,
            egui::TextEdit::singleline(&mut self.apple_id)
                .hint_text("name@example.com")
                .desired_width(field_w),
        );
        ui.add_space(12.0);

        let label = if running { lbl("[ RUNNING... ]", "Working…") } else { lbl("[ EXECUTE ]", "Start") };
        let blocker = self.start_blocker();
        let ready = blocker.is_none();
        let btn = egui::Button::new(RichText::new(label).color(th().ink).strong().size(16.0))
            .fill(if ready { th().accent } else { th().muted })
            .stroke(Stroke::new(1.0_f32, if ready { th().accent_dark } else { th().muted }))
            .corner_radius(CornerRadius::same(8));
        let resp = ui.add_enabled_ui(ready, |ui| ui.add_sized([field_w, 38.0], btn)).inner;
        let resp = match blocker {
            Some(why) => resp.on_disabled_hover_text(why),
            None => resp.on_hover_text("Start (Alt+S)"),
        };
        if resp.clicked() {
            self.try_start(false);
        }
    }

    fn info_card(&self, ui: &mut egui::Ui) {
        let Some(info) = &self.ipa else { return };
        card(ui, |ui| {
            egui::Grid::new("info").num_columns(4).spacing([14.0, 4.0]).show(ui, |ui| {
                kv(ui, "App", &info.name);
                kv(ui, "Bundle ID", &info.bundle_id);
                ui.end_row();
                let v = if info.build.is_empty() || info.build == info.version {
                    info.version.clone()
                } else {
                    format!("{} ({})", info.version, info.build)
                };
                kv(ui, "Version", &v);
                kv(ui, "Needs iOS", if info.min_ios.is_empty() { "—" } else { &info.min_ios });
                ui.end_row();
                kv(ui, "Size", &ipa::human_size(info.size_bytes));
                kv(ui, "Extensions", &info.extensions.to_string());
                ui.end_row();
            });
        });
    }

    fn advanced(&mut self, ui: &mut egui::Ui) {
        let running = self.run.is_some();
        // closed at start; Alt+V opens and closes it, like the original
        let arrow = if self.show_advanced { "⏷" } else { "⏵" };
        let head = egui::Button::new(
            RichText::new(format!("{arrow} {}", lbl("> ADVANCED_OPTIONS", "Advanced options"))).color(th().text),
        )
        .frame(false);
        if ui.add(head).on_hover_text("Show or hide the advanced options (Alt+V)").clicked() {
            self.show_advanced = !self.show_advanced;
        }
        if !self.show_advanced {
            return;
        }
        egui::Frame::default()
            .inner_margin(egui::Margin { left: 14, right: 0, top: 2, bottom: 4 })
            .show(ui, |ui| {
                ui.add_enabled_ui(!running, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Signing mode");
                        egui::ComboBox::from_id_salt("mode")
                            .width(220.0)
                            .selected_text(match self.mode {
                                Mode::Install => "Apple ID sideload (install)",
                                Mode::Export => "Export signed .ipa only",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.mode, Mode::Install, "Apple ID sideload (install)");
                                ui.selectable_value(&mut self.mode, Mode::Export, "Export signed .ipa only");
                            });
                    });
                    egui::Grid::new("adv_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label("App name");
                        let hint = self.ipa.as_ref().map(|i| i.name.clone()).unwrap_or_else(|| "keep original".into());
                        ui.add(egui::TextEdit::singleline(&mut self.custom_name).hint_text(hint).desired_width(260.0));
                        ui.end_row();
                        ui.label("Bundle ID");
                        let hint = self.ipa.as_ref().map(|i| i.bundle_id.clone()).unwrap_or_else(|| "keep original".into());
                        ui.add(egui::TextEdit::singleline(&mut self.custom_bundle).hint_text(hint).desired_width(260.0));
                        ui.end_row();
                    });
                    ui.checkbox(&mut self.remove_device_limits, "Remove limits on supported devices");
                    ui.checkbox(&mut self.file_sharing, "Let the app share files with the Files app");
                    ui.checkbox(&mut self.remove_extensions, "Remove app extensions (widgets, share sheets)");
                    ui.add_enabled(
                        !self.remove_extensions,
                        egui::Checkbox::new(&mut self.share_extension_ids, "Share the main app's ID with its extensions (uses fewer App IDs)"),
                    )
                    .on_hover_text("Free Apple IDs can make only 10 App IDs a week. Normally each extension gets its own; this makes them share one.");
                    ui.checkbox(&mut self.more_memory, "Ask iOS for a higher memory limit (emulators, big games)");
                    ui.checkbox(
                        &mut self.revoke_certs,
                        "If the certificate limit is reached, revoke an old certificate",
                    )
                    .on_hover_text("Free Apple IDs allow only a few signing certificates. Revoking one can stop apps signed with it.");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        use sideload::LocalAnisette as L;
                        ui.label("Sign-in data");
                        let name = |m: L| match m {
                            L::Auto => "Automatic (this PC first, then online)",
                            L::Only => "This PC only (iTunes/iCloud)",
                            L::Off => "Online servers only",
                        };
                        egui::ComboBox::from_id_salt("local_mode")
                            .width(260.0)
                            .selected_text(name(self.local_mode))
                            .show_ui(ui, |ui| {
                                for m in [L::Auto, L::Only, L::Off] {
                                    ui.selectable_value(&mut self.local_mode, m, name(m));
                                }
                            })
                            .response
                            .on_hover_text(
                                "Apple needs \"anisette\" sign-in data. \"This PC\" makes it with Apple's own iCloud for Windows \
                                 (web version) on this computer — the most trusted kind. Online servers are the backup.",
                            );
                    });
                    let local_note = match local_anisette::unavailable_reason() {
                        None => ("This PC's iCloud sign-in data: ready".to_string(), th().good),
                        Some(why) => (format!("This PC's iCloud sign-in data: not available — {why}"), th().warn),
                    };
                    ui.label(RichText::new(local_note.0).size(11.0).color(local_note.1));
                    ui.horizontal(|ui| {
                        ui.label("Online server");
                        let list = sideload::ANISETTE_SERVERS;
                        let name = |i: usize| -> String {
                            match i {
                                0 => "Automatic (tries several)".into(),
                                n if n <= list.len() => list[n - 1].trim_start_matches("https://").to_string(),
                                _ => "Custom…".into(),
                            }
                        };
                        egui::ComboBox::from_id_salt("anisette")
                            .width(220.0)
                            .selected_text(name(self.anisette_choice))
                            .show_ui(ui, |ui| {
                                for i in 0..=list.len() + 1 {
                                    ui.selectable_value(&mut self.anisette_choice, i, name(i));
                                }
                            })
                            .response
                            .on_hover_text("Apple needs extra sign-in data (\"anisette\") from one of these servers. If sign-in keeps saying the password is wrong, try another one.");
                    });
                    if self.anisette_choice == sideload::ANISETTE_SERVERS.len() + 1 {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.anisette_custom)
                                .hint_text("https://your-anisette-server")
                                .desired_width(320.0),
                        );
                    }
                    if ui
                        .button("Reset sign-in data")
                        .on_hover_text("Forget the saved sign-in data so fresh data is made next time. Use this if Apple says the password is wrong when it isn't.")
                        .clicked()
                    {
                        let n = sideload::reset_signin_data();
                        self.push(Kind::Good, format!("Sign-in data reset ({n} saved set(s) removed). Try signing in again."));
                    }
                    if ui
                        .button("Open saved log")
                        .on_hover_text("Every log line is also saved to a file (never your password), so problems can be looked at later.")
                        .clicked()
                    {
                        let path = log_file_path();
                        #[cfg(windows)]
                        let opened = std::process::Command::new("notepad.exe").arg(&path).spawn().is_ok();
                        #[cfg(not(windows))]
                        let opened = false;
                        if !opened {
                            self.push(Kind::Info, format!("Saved log: {}", path.display()));
                        }
                    }
                });
            });
    }

    /// The Help drop-down (top bar): a friendly guide in parts that open on
    /// click — what sideloading is, how to get an app file, how to use this
    /// app, and what to do if stuck. It floats over the page, so the main
    /// area stays as it is.
    fn help_menu(&mut self, ui: &mut egui::Ui) {
        if !self.help_seen {
            self.help_seen = true;
            if self.shot_path.is_none() {
                option_save("help-seen", "1");
            }
        }
        ui.set_width(390.0);
        ui.label(RichText::new("How it works").color(th().accent).strong().size(15.0));
        ui.label(RichText::new("Click a title to open it.").color(th().muted).size(12.0));
        ui.add_space(2.0);
        // room from here down to the window's bottom edge; asked for explicitly,
        // because a pop-up otherwise keeps the size it had when it first opened
        let max_h = (ui.ctx().screen_rect().bottom() - ui.cursor().top() - 28.0).max(160.0);
        egui::ScrollArea::vertical()
            .id_salt("help_scroll")
            .max_height(max_h)
            .min_scrolled_height(max_h)
            .auto_shrink([false, true])
            .show(ui, |ui| {

                help_part(ui, "help_what", "What is sideloading?", true, |ui| {
                    help_text(
                        ui,
                        "Normally, iPhone and iPad apps come from the App Store. Sideloading means putting \
                         an app on your phone yourself, from your computer, without the App Store.",
                    );
                    help_points(
                        ui,
                        &[
                            "An iPhone app comes as one file whose name ends in .ipa. Think of it as the app in a box.",
                            "iPhones only open apps that are signed (approved). This app signs the .ipa with your own \
                             free Apple ID, so your phone accepts it.",
                            "With a free Apple ID, a sideloaded app works for 7 days. Then just install it again with \
                             this app; what you saved inside the app stays.",
                            "A free Apple ID can have up to 3 sideloaded apps on a phone at once.",
                        ],
                    );
                });

                help_part(ui, "help_get", "Getting an app file (.ipa), including tweaked apps", false, |ui| {
                    help_text(
                        ui,
                        "A tweaked app is a changed version of an app with extra features. Here is how to get the file:",
                    );
                    help_steps(
                        ui,
                        &[
                            "Look up the app's name plus \"IPA\" on the web, for example: \"app name IPA\" or \
                             \"app name tweaked IPA\".",
                            "Find the .ipa file for that app and download it to your computer. It usually ends up \
                             in your Downloads folder.",
                            "If it came inside a .zip file, open the zip and take out the file that ends in .ipa.",
                            "Now drop that .ipa file into this app (see \"Using this app\" below).",
                        ],
                    );
                    help_text(
                        ui,
                        "Stay safe: only download from sites and people you trust, let your antivirus check the \
                         file, and only install apps you're allowed to use. A file from an unknown place can hide \
                         harmful code.",
                    );
                });

                help_part(ui, "help_use", "Using this app, step by step", false, |ui| {
                    help_text(
                        ui,
                        "Once per computer: install iTunes and iCloud for Windows from apple.com (the web \
                         versions, not the Microsoft Store ones).",
                    );
                    help_steps(
                        ui,
                        &[
                            "Plug your iPhone or iPad into the computer with a cable. Unlock it, and if it asks \
                             \"Trust This Computer?\", tap Trust and enter your passcode.",
                            "Drop the .ipa file onto the big box on the left (or click the box to choose the file).",
                            "Check that your phone's name shows under \"Device\".",
                            "Type your Apple ID email under \"Apple ID\". A spare Apple ID is the safest choice.",
                            "Press Start. Type your Apple ID password when asked; it goes only to Apple and is never \
                             written to the log. If Apple sends you a 6-digit code, type it in.",
                            "Wait until the bar at the bottom is full and says it's done. \"☰ History\" shows every step.",
                            "On the phone, open Settings › General › VPN & Device Management, tap your Apple ID, \
                             then tap Trust.",
                            "On iOS 16 or newer, turn on Developer Mode: Settings › Privacy & Security › Developer \
                             Mode, then restart the phone when it asks.",
                            "Open the app on your phone and enjoy. After 7 days, repeat steps 1 to 6 to renew it.",
                        ],
                    );
                });

                help_part(ui, "help_fix", "If something goes wrong", false, |ui| {
                    help_points(
                        ui,
                        &[
                            "No phone under \"Device\": unplug and plug the cable back in, unlock the phone, tap \
                             Trust, and make sure iTunes from apple.com is installed.",
                            "Apple says the password is wrong but it's right: open Advanced options, press \"Reset \
                             sign-in data\", then try again.",
                            "The app won't open on the phone: trust your Apple ID and turn on Developer Mode \
                             (steps 7 and 8 above).",
                            "Asked to pick a certificate to remove: that's normal for free Apple IDs; pick an old one.",
                            "Want help from someone? Press \"☰ History\" (or Alt+.), then \"Copy\", and paste the log \
                             to them.",
                        ],
                    );
                });
            });
    }

    fn log_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(lbl("> TERMINAL", "Log")).color(th().muted).size(12.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("Clear").clicked() {
                    self.log.clear();
                }
                if ui.small_button("Copy").on_hover_text("Copy the whole log").clicked() {
                    ui.ctx().copy_text(self.log_text());
                }
                if ui.small_button("Saved log").on_hover_text("Open the saved log file (includes extra detail)").clicked() {
                    open_log_file();
                }
            });
        });
        egui::Frame::default()
            .fill(th().card)
            .stroke(Stroke::new(1.0_f32, th().border))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for (kind, text) in &self.log {
                            let color = match kind {
                                Kind::Info => th().text,
                                Kind::Good => th().good,
                                Kind::Warn => th().warn,
                                Kind::Bad => th().bad,
                            };
                            ui.label(RichText::new(format!("{}{text}", lbl("> ", ""))).monospace().size(12.0).color(color));
                        }
                    });
            });
    }

    fn password_window(&mut self, ctx: &egui::Context, now: f64) {
        if !self.asking_password {
            return;
        }
        // dim the window behind
        let screen = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("dim")))
            .rect_filled(screen, 0.0, Color32::from_black_alpha(90));
        let mut go = false;
        let mut cancel = false;
        egui::Window::new("Apple ID password")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([330.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Enter the password for {}", self.apple_id.trim()));
                ui.add_space(4.0);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.password)
                        .password(!self.show_password)
                        .hint_text("Password")
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    go = true;
                }
                ui.checkbox(&mut self.show_password, "Show password (check for typos)");
                let letters: Vec<char> = self.password.chars().filter(|c| c.is_alphabetic()).collect();
                if letters.len() >= 3 && letters.iter().all(|c| c.is_uppercase()) {
                    ui.label(RichText::new("All capitals — is Caps Lock on?").size(11.0).color(th().warn));
                }
                if self.password != self.password.trim() {
                    ui.label(RichText::new("Starts or ends with a space — check that's right.").size(11.0).color(th().warn));
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Sent only to Apple to sign in. It is not saved.")
                        .size(11.0)
                        .color(th().muted),
                );
                ui.add_space(6.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = egui::Button::new(RichText::new("Continue").color(th().ink)).fill(th().accent);
                    if ui.add_enabled(!self.password.is_empty(), ok).clicked() {
                        go = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        let _ = now;
        if go && !self.password.is_empty() {
            self.asking_password = false;
            self.begin_run(ctx);
            self.password.clear();
        } else if cancel {
            self.asking_password = false;
            self.password.clear();
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.shot_path.clone() else { return };
        self.frames += 1;
        ctx.request_repaint();
        // Only ask for the picture when the window really has a size: a
        // zero-sized (e.g. momentarily minimised) window crashes the reader.
        let sized = ctx.input(|i| {
            let vp = i.viewport();
            !vp.minimized.unwrap_or(false)
                && vp.inner_rect.map(|r| r.width() >= 1.0 && r.height() >= 1.0).unwrap_or(false)
                && i.screen_rect.width() >= 1.0
                && i.screen_rect.height() >= 1.0
        });
        if self.frames >= 120 && sized && !self.shot_sent_recently {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.shot_sent_recently = true;
            self.shot_sent_frame = self.frames;
        }
        if self.shot_sent_recently && self.frames - self.shot_sent_frame >= 60 {
            self.shot_sent_recently = false; // no picture arrived; try again
        }
        let image = ctx.input(|i| {
            i.events.iter().rev().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(buf) = image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, bytes) {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = buf.save(&path) {
                    eprintln!("could not save screenshot: {e}");
                }
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.frames > 1200 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for LoaderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);

        // device auto-detect every few seconds
        self.poll_scan();
        if self.run.is_none() && now - self.last_scan > 4.0 {
            self.start_scan(now);
        }
        self.tick_run(now);

        // drag & drop
        let hovering_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if let Some(p) = dropped.into_iter().next() {
            self.load_ipa(p);
        }

        // the original's keys: Alt+S starts, Alt+. shows/hides the status
        // history, Alt+V opens/closes Advanced options
        let (alt_s, alt_dot, alt_v) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::ALT, egui::Key::S),
                i.consume_key(egui::Modifiers::ALT, egui::Key::Period),
                i.consume_key(egui::Modifiers::ALT, egui::Key::V),
            )
        });
        let dialog_open = self.asking_password || self.code_req.is_some() || self.cert_req.is_some();
        if alt_s && !dialog_open && self.start_blocker().is_none() {
            self.try_start(false);
        }
        if alt_v {
            self.show_advanced = !self.show_advanced;
        }
        let mut toggle_history = alt_dot;

        // full screen: F11 toggles, Esc leaves (unless the password box is open)
        let mut toggle_full = ctx.input(|i| i.key_pressed(egui::Key::F11));
        let is_full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        if is_full && !self.asking_password && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            toggle_full = true;
        }

        // the theme's background (hacker rain, stars, bubbles…) behind everything
        theme::paint_backdrop(ctx, now);

        let mut help_open = cfg!(debug_assertions) && std::env::var_os("IPABAZOOKA_PREVIEW_HELP").is_some();
        let header_h = egui::TopBottomPanel::top("header")
            .frame(egui::Frame::default().fill(th().card).inner_margin(egui::Margin::symmetric(12, 5)).stroke(Stroke::new(1.0_f32, th().border)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if th().terminal {
                        let cursor = if (now * 2.0) as i64 % 2 == 0 { "█" } else { " " };
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("root@ipabazooka").color(th().accent).strong().size(13.0));
                                ui.label(RichText::new(format!(":~# ./sideload{cursor}")).color(th().text).size(13.0));
                            });
                            ui.label(RichText::new(format!("# {CREDIT}")).color(th().accent).size(10.5));
                        });
                    } else {
                        ui.label(RichText::new("ipabazooka").color(th().accent).strong().size(15.0));
                        // the tagline with the maker's credit right under it
                        // (the bar is too narrow for both on one line)
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.label(RichText::new("install apps on your iPhone").color(th().muted).size(11.0));
                            ui.label(RichText::new(CREDIT).color(th().accent).strong().size(10.5));
                        });
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // theme picker
                        let current = theme::current_index();
                        let mut picked = current;
                        egui::ComboBox::from_id_salt("theme")
                            .width(150.0)
                            .height(460.0)
                            .selected_text(format!("{} {}", th().icon, th().name))
                            .show_ui(ui, |ui| {
                                for (i, t) in theme::THEMES.iter().enumerate() {
                                    ui.selectable_value(&mut picked, i, format!("{} {}", t.icon, t.name));
                                }
                            })
                            .response
                            .on_hover_text("Choose the look of the app");
                        if picked != current {
                            theme::select(ctx, picked);
                        }
                        ui.label(RichText::new(lbl("THEME", "Theme")).color(th().muted).size(11.0));
                        ui.add_space(8.0);
                        // the guide: a drop-down off to the side, drawn as a bold
                        // white button on a strong colour so it stands out
                        let menu = ui.scope(|ui| {
                            let fill = theme::fill_for_white_text(th().accent);
                            let hover = theme::lighten(fill, 0.15);
                            let w = &mut ui.visuals_mut().widgets;
                            for (state, bg) in [
                                (&mut w.inactive, fill),
                                (&mut w.hovered, hover),
                                (&mut w.active, hover),
                                (&mut w.open, hover),
                            ] {
                                state.weak_bg_fill = bg;
                                state.bg_fill = bg;
                                state.bg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
                                state.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
                                state.corner_radius = CornerRadius::same(6);
                            }
                            ui.spacing_mut().button_padding = Vec2::new(10.0, 3.0);
                            let help_label = RichText::new(lbl("[ HELP ]", "❓ Help"))
                                .color(Color32::WHITE)
                                .strong()
                                .size(14.0);
                            ui.menu_button(help_label, |ui| self.help_menu(ui))
                        })
                        .inner;
                        help_open |= menu.inner.is_some();
                        menu.response
                            .on_hover_text("New here? How sideloading works and how to use this app");
                        ui.add_space(6.0);
                        let (txt, col) = match self.service_ok {
                            Some(true) if !self.devices.is_empty() => (lbl("[ LINK ESTABLISHED ]", "Device connected"), th().good),
                            Some(true) => (lbl("[ AWAITING TARGET ]", "Waiting for device"), th().warn),
                            Some(false) => (lbl("[ NO USB SERVICE ]", "Apple device service missing"), th().bad),
                            None => (lbl("[ SCANNING ]", "Looking for devices"), th().muted),
                        };
                        // only when it fits beside the title (the device box says it too)
                        let txt = RichText::new(txt).color(col).size(11.0);
                        let needed = egui::WidgetText::from(txt.clone())
                            .into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body)
                            .size()
                            .x;
                        // (the title is already placed, so what's left is the gap)
                        if ui.available_width() > needed + 16.0 {
                            ui.label(txt);
                        }
                    });
                });
            })
            .response
            .rect
            .height();

        // status row: newest log line (or the running steps) · percent · bar ·
        // status history toggle · full screen
        let footer_h = egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::default().fill(th().card).inner_margin(egui::Margin::symmetric(12, 6)).stroke(Stroke::new(1.0_f32, th().border)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let show_bar = self.run.is_some() || self.bar.value > 0;
                    let right_w = if show_bar { 380.0 } else { 220.0 };
                    let text_w = (ui.available_width() - right_w).max(60.0);
                    let c = match (self.run.is_some(), self.last_result) {
                        (false, Some(true)) => th().good,
                        (false, Some(false)) => th().bad,
                        _ => th().text,
                    };
                    ui.allocate_ui_with_layout(Vec2::new(text_w, 18.0), Layout::left_to_right(Align::Center), |ui| {
                        ui.add(egui::Label::new(RichText::new(&self.status).color(c).size(12.0)).truncate())
                            .on_hover_text(&self.status);
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                        let label = if full { "⛶ Exit full screen" } else { "⛶ Full screen" };
                        if ui
                            .small_button(label)
                            .on_hover_text("Toggle full screen (F11)")
                            .clicked()
                        {
                            toggle_full = true;
                        }
                        let tip = if self.show_history {
                            "Hide status history (Alt+.)"
                        } else {
                            "Show status history (Alt+.)"
                        };
                        let btn = egui::Button::new("☰ History").small().selected(self.show_history);
                        if ui.add(btn).on_hover_text(tip).clicked() {
                            toggle_history = !toggle_history;
                        }
                        if show_bar {
                            ui.add_space(6.0);
                            let (bar, _) = ui.allocate_exact_size(Vec2::new(150.0, 8.0), Sense::hover());
                            let p = ui.painter();
                            p.rect_filled(bar, CornerRadius::same(4), th().w_inactive);
                            let frac = self.bar.fraction();
                            if frac > 0.0 {
                                let mut f = bar;
                                f.set_width((bar.width() * frac).max(8.0));
                                p.rect_filled(f, CornerRadius::same(4), if frac >= 1.0 { th().good } else { th().accent });
                            }
                            ui.label(RichText::new(format!("{}%", self.bar.percent())).color(th().muted).size(11.0));
                        }
                    });
                });
            })
            .response
            .rect
            .height();

        let mut content_h = 0.0;
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(Color32::TRANSPARENT).inner_margin(egui::Margin::same(16)))
            .show(ctx, |ui| {
                // With the history open, the upper part scrolls when it gets
                // tall, so the log below always keeps ~200 points of room.
                let top_max = if self.show_history { (ui.available_height() - 200.0).max(140.0) } else { f32::INFINITY };
                let out = egui::ScrollArea::vertical()
                    .id_salt("top_part")
                    .max_height(top_max)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            self.drop_tile(ui, hovering_files);
                            ui.add_space(14.0);
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width());
                                self.right_column(ui, now);
                            });
                        });
                        ui.add_space(10.0);
                        self.info_card(ui);
                        self.advanced(ui);
                    });
                content_h = out.content_size.y;
                if self.show_history {
                    ui.add_space(4.0);
                    self.log_view(ui);
                }
            });

        // Like the original, the window's height follows its content while the
        // history is hidden (refitted whenever the content changes height).
        let (maxed, inner, monitor) = ctx.input(|i| {
            let vp = i.viewport();
            (vp.maximized.unwrap_or(false), vp.inner_rect.map(|r| r.size()), vp.monitor_size)
        });
        // While the Help drop-down is open, give it room to read (it floats
        // over the page); when it closes, the window goes back to its size.
        if help_open && !is_full && !maxed {
            if let (Some(inner), None) = (inner, self.help_restore_h) {
                let cap = monitor.map(|m| m.y - 60.0).unwrap_or(1000.0).max(MIN_WINDOW_H);
                let want = HELP_ROOM_H.min(cap);
                if inner.y + 1.0 < want {
                    self.help_restore_h = Some(inner.y);
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(inner.x, want)));
                }
            }
        } else if !help_open {
            if let (Some(h), Some(inner)) = (self.help_restore_h.take(), inner) {
                if !is_full && !maxed {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(inner.x, h)));
                }
                self.last_fit_h = 0.0;
            }
        }
        if !help_open && !self.show_history && !is_full && !maxed && content_h > 0.0 {
            if let Some(inner) = inner {
                let cap = monitor.map(|m| m.y - 60.0).unwrap_or(1000.0).max(MIN_WINDOW_H);
                let want = (header_h + footer_h + content_h + 34.0).clamp(MIN_WINDOW_H, cap).round();
                if (want - self.last_fit_h).abs() > 1.0 {
                    self.last_fit_h = want;
                    if (want - inner.y).abs() > 1.0 {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(inner.x, want)));
                    }
                }
            }
        }
        if toggle_history {
            self.set_history(ctx, !self.show_history);
        }

        // debug builds only: show the Help drop-down's contents for a test picture
        #[cfg(debug_assertions)]
        if std::env::var_os("IPABAZOOKA_PREVIEW_HELP").is_some() {
            let x = ctx.screen_rect().right() - 410.0;
            egui::Area::new(egui::Id::new("help_preview"))
                .fixed_pos(egui::pos2(x, header_h + 2.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::menu(ui.style()).show(ui, |ui| self.help_menu(ui));
                });
        }

        self.password_window(ctx, now);
        self.code_window(ctx);
        self.cert_window(ctx);
        self.phone_window(ctx);
        self.failure_window(ctx);

        if toggle_full {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_full));
        }

        if self.run.is_some() || self.scan_rx.is_some() {
            ctx.request_repaint();
        } else {
            // moving backgrounds redraw ~30 times a second; still ones rarely
            let ms = if theme::animated() { 33 } else { 500 };
            ctx.request_repaint_after(std::time::Duration::from_millis(ms));
        }
        self.handle_screenshot(ctx);
    }
}

/// Apple IDs are emails and not case-sensitive: trim spaces, drop invisible
/// characters that sneak in when copying, and use lower case.
fn normalize_apple_id(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && !matches!(*c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' | '\u{00A0}'))
        .collect::<String>()
}

/// The maker's credit shown in the top bar.
const CREDIT: &str = "created by CyberWithSkyer";

/// Starting window width in points: the original's 750 px layout width × 8/10
/// at normal (96 DPI) scaling; egui points already follow the screen's scaling.
const ORIGINAL_LAYOUT_W: f32 = 750.0;
const WINDOW_W: f32 = ORIGINAL_LAYOUT_W * 8.0 / 10.0;
/// Shortest the window gets when it fits its content.
const MIN_WINDOW_H: f32 = 300.0;
/// Extra height given to the status history when it is shown.
const HISTORY_ROOM: f32 = 260.0;
/// The window is at least this tall while the Help drop-down is open.
const HELP_ROOM_H: f32 = 620.0;
/// Saved options are read up to this many bytes, like the original.
const OPTION_MAX_BYTES: usize = 512;

/// Each saved option is its own small file: %APPDATA%\ipabazooka\option-<name>
fn option_path(name: &str) -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("ipabazooka")
        .join(format!("option-{name}"))
}

/// None when the option was never saved.
fn option_load(name: &str) -> Option<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(option_path(name)).ok()?;
    let mut buf = vec![0u8; OPTION_MAX_BYTES];
    let n = f.read(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf[..n]).into_owned())
}

fn option_save(name: &str, value: &str) {
    let path = option_path(name);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, value);
}

/// Where the saved log goes: %APPDATA%\ipabazooka\ipabazooka.log
fn log_file_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("ipabazooka")
        .join("ipabazooka.log")
}

/// Appends one log line (with UTC time) to the saved log. Passwords are
/// never put in the log. The file is cut back when it grows past ~1 MB.
fn write_log_file(kind: Kind, text: &str) {
    let tag = match kind {
        Kind::Info => "INFO",
        Kind::Good => "OK  ",
        Kind::Warn => "WARN",
        Kind::Bad => "FAIL",
    };
    write_log_line(tag, text);
}

/// Appends lines to the saved log with a date and time (UTC) and a tag
/// (INFO / OK / WARN / FAIL, plus DBG and RAW for the detailed lines that
/// only go to the file). The file is UTF-8 with a marker so Notepad and
/// other viewers show symbols correctly. Past ~2 MB the old log is kept as
/// ipabazooka.old.log and a new one starts. Passwords are never logged.
pub(crate) fn write_log_line(tag: &str, text: &str) {
    use std::io::Write;
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock();
    let path = log_file_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    static CHECKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let mut rotated = false;
    if std::fs::metadata(&path).map(|m| m.len() > 2_000_000).unwrap_or(false) {
        let _ = std::fs::rename(&path, path.with_file_name("ipabazooka.old.log"));
        rotated = true;
    }
    // make sure the file starts with the UTF-8 marker (checked once per run
    // and after rotating, not on every line)
    let needs_bom = if rotated || !CHECKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        use std::io::Read;
        let mut head = [0u8; 3];
        match std::fs::File::open(&path) {
            Ok(mut f) => f.read_exact(&mut head).is_err() || head != [0xEF, 0xBB, 0xBF],
            Err(_) => true,
        }
    } else {
        false
    };
    if needs_bom {
        let old = std::fs::read(&path).unwrap_or_default();
        let mut fresh = vec![0xEF, 0xBB, 0xBF];
        fresh.extend_from_slice(&old);
        let _ = std::fs::write(&path, fresh);
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let stamp = isideload::anisette::format_utc(secs).replace('T', " ").replace('Z', " UTC");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        for line in text.lines() {
            let _ = writeln!(f, "[{stamp}] {tag} {line}");
        }
    }
}

/// Opens the saved log in Notepad (Windows) or says where it is.
fn open_log_file() {
    let path = log_file_path();
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("notepad.exe").arg(&path).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = path;
    }
}


fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::default()
        .fill(th().card)
        .stroke(Stroke::new(1.0_f32, th().border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
    ui.add_space(6.0);
}

/// One drop-down part of the "New here?" guide.
fn help_part(ui: &mut egui::Ui, id: &str, title: &str, open: bool, body: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).color(th().text).strong().size(14.0))
        .id_salt(id)
        .default_open(open)
        .show(ui, |ui| {
            body(ui);
            ui.add_space(4.0);
        });
}

fn help_text(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).color(th().text).size(13.0)).wrap());
    ui.add_space(3.0);
}

/// Numbered steps.
fn help_steps(ui: &mut egui::Ui, steps: &[&str]) {
    for (i, s) in steps.iter().enumerate() {
        help_text(ui, &format!("{}.  {s}", i + 1));
    }
}

/// Bullet points.
fn help_points(ui: &mut egui::Ui, points: &[&str]) {
    for p in points {
        help_text(ui, &format!("•  {p}"));
    }
}

fn kv(ui: &mut egui::Ui, k: &str, v: &str) {
    ui.label(RichText::new(k).color(th().muted).size(12.0));
    ui.label(RichText::new(shorten(v, 34)).color(th().text).size(12.0));
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max - 1).collect();
        format!("{t}…")
    }
}

/// Window/taskbar icon: a blue rounded square with a white down arrow.
fn make_icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    let r = 14.0f32;
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // rounded-square mask
            let cx = fx.clamp(r, n as f32 - r);
            let cy = fy.clamp(r, n as f32 - r);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let a = (r + 0.5 - d).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            let t = fy / n as f32;
            let mut c = [0u8, (200.0 - 60.0 * t) as u8, (60.0 - 20.0 * t) as u8];
            // arrow: stem + head + tray
            let stem = (fx - 32.0).abs() < 4.5 && fy > 14.0 && fy < 36.0;
            let head = fy >= 30.0 && fy < 44.0 && (fx - 32.0).abs() < (44.0 - fy) * 1.1;
            let tray = fy > 47.0 && fy < 52.0 && fx > 16.0 && fx < 48.0;
            if stem || head || tray {
                c = [0, 12, 2];
            }
            let i = (y * n + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], (a * 255.0) as u8]);
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}

fn main() -> eframe::Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let _ = isideload::init();
    sideload::init_logging();
    let args: Vec<String> = std::env::args().collect();
    let shot_path = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    // --theme <hacker|normal|midnight|sunset|ocean> picks a look for this run only
    let theme_arg = args
        .iter()
        .position(|a| a == "--theme")
        .and_then(|i| args.get(i + 1))
        .cloned();
    // "Open with" / double-click support: any .ipa given on the command line is loaded.
    let open_path = args
        .iter()
        .skip(1)
        .find(|a| a.to_lowercase().ends_with(".ipa"))
        .map(PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ipabazooka")
        .with_inner_size([WINDOW_W, 520.0])
        .with_min_inner_size([520.0, MIN_WINDOW_H])
        .with_drag_and_drop(true)
        .with_icon(std::sync::Arc::new(make_icon()));
    if shot_path.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if shot_path.is_some() {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        options.event_loop_builder = Some(Box::new(|b| {
            b.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "ipabazooka",
        options,
        Box::new(move |cc| {
            let mut app = LoaderApp::new(shot_path);
            theme::load_saved();
            if let Some(key) = &theme_arg {
                theme::use_key(key);
            }
            theme::apply(&cc.egui_ctx);
            if let Some(p) = open_path {
                app.load_ipa(p);
            }
            Ok(Box::new(app))
        }),
    )
}
