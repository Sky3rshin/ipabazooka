//! Progress made of named steps, following the rules in MECHANICS.md
//! ("Progress"): each step's share only grows and is capped at 100 %, the
//! overall bar is the weighted average of the steps (weight = a step's
//! total) out of 10000 and never goes back, and each step keeps one log line
//! that is updated in place while its detail stays the same.

use std::collections::HashMap;

/// The bar's maximum (the original's progress bar runs to 10000).
pub const BAR_MAX: u32 = 10_000;

/// Joins the label of the running steps and the latest detail.
pub const LABEL_DETAIL_SEP: &str = "\u{2026} ";

#[derive(Default)]
struct Step {
    done: u64,
    total: u64,
    ratio: f64,
    active: bool,
    logged_line: String,
    logged_detail: String,
    logged_pct: String,
}

/// What to write in the log for one progress message.
#[derive(Debug, PartialEq)]
pub struct LogChange {
    pub line: String,
    /// Replace this earlier line (the same step, same detail) instead of adding one.
    pub replaces: Option<String>,
}

/// The bar and its label, as the window shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// The running steps that have started, joined with " & ".
    pub steps: String,
    /// The latest detail any step gave.
    pub detail: String,
    /// 0..=BAR_MAX
    pub value: u32,
}

impl View {
    /// "steps… detail", or whichever of the two there is.
    pub fn label(&self) -> String {
        match (self.steps.is_empty(), self.detail.is_empty()) {
            (false, false) => format!("{}{LABEL_DETAIL_SEP}{}", self.steps, self.detail),
            (false, true) => self.steps.clone(),
            _ => self.detail.clone(),
        }
    }
    /// Whole percent, rounded down (value × 100 / max).
    pub fn percent(&self) -> u32 {
        percent_text_value(self.value as u64, BAR_MAX as u64)
    }
    pub fn fraction(&self) -> f32 {
        self.value as f32 / BAR_MAX as f32
    }
}

/// value × 100 / max with whole numbers (0 when there is no maximum).
pub fn percent_text_value(value: u64, max: u64) -> u32 {
    if max == 0 {
        0
    } else {
        (value * 100 / max) as u32
    }
}

/// Splits "Step: detail" or "Step… detail" into its step and detail; the
/// separator that comes first wins. Without one, the whole text is the step.
pub fn split_message(msg: &str) -> (String, String) {
    let padded = format!("{msg} ");
    let colon = padded.find(": ").map(|i| i.min(msg.len()));
    let dots = msg.find('\u{2026}');
    let cut = match (colon, dots) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    let Some(i) = cut else {
        return (msg.trim().to_string(), String::new());
    };
    let step = msg[..i].trim().to_string();
    let rest = &msg[i..];
    let rest = rest
        .strip_prefix(':')
        .or_else(|| rest.strip_prefix('\u{2026}'))
        .unwrap_or(rest);
    (step, rest.trim().to_string())
}

#[derive(Default)]
pub struct Tracker {
    steps: HashMap<String, Step>,
    order: Vec<String>,
    latest_detail: String,
    last_overall: f64,
    have_overall: bool,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// One progress message: `done` of `total` for the step it names.
    /// Returns the log change it causes, if any.
    pub fn update(&mut self, msg: &str, done: u64, total: u64) -> Option<LogChange> {
        let (name, detail) = split_message(msg);
        let is_new = !self.steps.contains_key(&name);
        if is_new {
            self.order.push(name.clone());
        }
        {
            let step = self.steps.entry(name.clone()).or_default();
            step.done = done;
            step.total = total;
            step.active = total == 0 || done < total;
        }
        if !detail.is_empty() {
            self.latest_detail = detail.clone();
        }
        self.refresh();

        let step = self.steps.get_mut(&name)?;
        let pct = format!("{}%", (100.0 * step.ratio + 0.5) as i64);
        let detail_changed = is_new || step.logged_detail != detail;
        if step.done == 0 || !(detail_changed || step.logged_pct != pct) {
            return None;
        }
        let mut line = format!("{name}: {pct}");
        if !detail.is_empty() {
            line.push(' ');
            line.push_str(&detail);
        }
        let replaces = if !detail_changed && !step.logged_line.is_empty() {
            Some(std::mem::take(&mut step.logged_line))
        } else {
            None
        };
        step.logged_line = line.clone();
        step.logged_detail = detail;
        step.logged_pct = pct;
        Some(LogChange { line, replaces })
    }

    /// Updates each step's share (never down, at most 1) and the overall value.
    fn refresh(&mut self) -> f64 {
        let (mut weight_sum, mut sum) = (0.0f64, 0.0f64);
        for name in &self.order {
            let Some(step) = self.steps.get_mut(name) else { continue };
            if step.total != 0 {
                let now = step.done as f64 / step.total as f64;
                if step.ratio <= now {
                    step.ratio = now.min(1.0);
                }
            }
            let weight = if step.total == 0 { step.done as f64 } else { step.total as f64 };
            weight_sum += weight;
            sum += step.ratio * weight;
        }
        let overall = if weight_sum <= 0.0 { 0.0 } else { sum / weight_sum };
        let overall = if self.have_overall && self.last_overall > overall { self.last_overall } else { overall };
        self.last_overall = overall;
        self.have_overall = true;
        overall
    }

    pub fn view(&self) -> View {
        let steps: Vec<&str> = self
            .order
            .iter()
            .filter(|n| self.steps.get(*n).map(|s| s.active && s.done != 0).unwrap_or(false))
            .map(|n| n.as_str())
            .collect();
        View {
            steps: steps.join(" & "),
            detail: self.latest_detail.clone(),
            value: (0.5 + BAR_MAX as f64 * self.last_overall) as u32,
        }
    }

    /// Everything finished: the bar is full.
    pub fn finish(&mut self) {
        self.last_overall = 1.0;
        self.have_overall = true;
        for s in self.steps.values_mut() {
            s.active = false;
        }
    }
}

/// The original's status line: the newest log line, cut to 128 bytes with "…".
pub fn status_line(text: &str) -> String {
    const MAX: usize = 128;
    let first = text.lines().next().unwrap_or("");
    if first.len() <= MAX {
        return first.to_string();
    }
    let mut cut = MAX;
    while !first.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}\u{2026}", &first[..cut])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_the_first_separator() {
        assert_eq!(split_message("Signing: Getting certificate"), ("Signing".into(), "Getting certificate".into()));
        assert_eq!(split_message("Uploading\u{2026} app.ipa"), ("Uploading".into(), "app.ipa".into()));
        assert_eq!(split_message("  Signing in  "), ("Signing in".into(), String::new()));
        assert_eq!(split_message("Step:"), ("Step".into(), String::new()));
        assert_eq!(split_message("A\u{2026} b: c"), ("A".into(), "b: c".into()));
    }

    #[test]
    fn weighted_and_never_backwards() {
        let mut t = Tracker::new();
        t.update("Signing in", 0, 10);
        t.update("Installing", 0, 30);
        assert_eq!(t.view().value, 0);
        t.update("Signing in", 10, 10);
        // 10 of 40 weight done
        assert_eq!(t.view().value, 2500);
        assert_eq!(t.view().percent(), 25);
        // a step can't go back, and neither can the bar
        t.update("Signing in", 2, 10);
        assert_eq!(t.view().value, 2500);
        t.update("Installing", 15, 30);
        assert_eq!(t.view().value, 6250);
        // more than the total is capped at 100 %
        t.update("Installing", 90, 30);
        assert_eq!(t.view().value, BAR_MAX);
    }

    #[test]
    fn label_lists_running_steps() {
        let mut t = Tracker::new();
        t.update("Signing", 0, 60);
        t.update("Installing", 0, 100);
        assert_eq!(t.view().label(), "");
        t.update("Signing: Getting certificate", 6, 60);
        assert_eq!(t.view().label(), "Signing\u{2026} Getting certificate");
        t.update("Installing", 5, 100);
        assert_eq!(t.view().steps, "Signing & Installing");
        t.update("Signing", 60, 60);
        assert_eq!(t.view().steps, "Installing");
    }

    #[test]
    fn one_log_line_per_step_and_detail() {
        let mut t = Tracker::new();
        assert_eq!(t.update("Installing", 0, 100), None);
        let a = t.update("Installing", 10, 100).unwrap();
        assert_eq!(a, LogChange { line: "Installing: 10%".into(), replaces: None });
        // same percent: nothing new
        assert_eq!(t.update("Installing", 10, 100), None);
        let b = t.update("Installing", 50, 100).unwrap();
        assert_eq!(b, LogChange { line: "Installing: 50%".into(), replaces: Some("Installing: 10%".into()) });
        // a new detail gets its own line
        let c = t.update("Installing: Verifying", 60, 100).unwrap();
        assert_eq!(c, LogChange { line: "Installing: 60% Verifying".into(), replaces: None });
    }

    #[test]
    fn percent_rounds_down() {
        assert_eq!(percent_text_value(9999, 10000), 99);
        assert_eq!(percent_text_value(5, 0), 0);
    }

    #[test]
    fn status_line_is_cut_at_128_bytes() {
        let long = "x".repeat(200);
        let s = status_line(&long);
        assert_eq!(s.len(), 128 + "\u{2026}".len());
        assert!(s.ends_with('\u{2026}'));
        assert_eq!(status_line("short\nsecond"), "short");
    }
}
