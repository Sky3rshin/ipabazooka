//! "Phone tools": things done directly on the plugged-in iPhone/iPad —
//! list the apps you installed, remove one, check Developer Mode and make
//! its switch appear in Settings. Each job runs on its own thread.

use idevice::IdeviceService;
use std::sync::mpsc::Sender;

#[derive(Clone)]
pub struct PhoneApp {
    pub bundle_id: String,
    pub name: String,
    pub version: String,
    /// installed with a development (free Apple ID) signature
    pub sideloaded: bool,
}

pub enum PhoneJob {
    ListApps,
    Remove(String),
    DevModeStatus,
    RevealDevMode,
}

pub enum PhoneEvent {
    Apps(Result<Vec<PhoneApp>, String>),
    DevMode(Result<bool, String>),
    /// a finished action: (what, result message)
    Done(String, Result<String, String>),
}

async fn provider(udid: &str) -> Result<idevice::provider::UsbmuxdProvider, String> {
    let mut mux = idevice::usbmuxd::UsbmuxdConnection::default()
        .await
        .map_err(|e| format!("Apple device service not reachable: {e}"))?;
    let dev = mux
        .get_device(udid)
        .await
        .map_err(|e| format!("Phone not found (unplugged?): {e}"))?;
    Ok(dev.to_provider(idevice::usbmuxd::UsbmuxdAddr::default(), "ipabazooka"))
}

fn text(d: &plist::Dictionary, k: &str) -> String {
    d.get(k).and_then(|v| v.as_string()).unwrap_or("").to_string()
}

async fn list_apps(udid: &str) -> Result<Vec<PhoneApp>, String> {
    let p = provider(udid).await?;
    let mut ip = idevice::installation_proxy::InstallationProxyClient::connect(&p)
        .await
        .map_err(|e| pairing_hint(format!("Couldn't open the phone's app list: {e}")))?;
    let apps = ip
        .get_apps(Some("User"), None)
        .await
        .map_err(|e| format!("Couldn't read the phone's app list: {e}"))?;
    let mut out: Vec<PhoneApp> = apps
        .into_iter()
        .map(|(id, v)| {
            let d = v.as_dictionary().cloned().unwrap_or_default();
            let mut name = text(&d, "CFBundleDisplayName");
            if name.is_empty() {
                name = text(&d, "CFBundleName");
            }
            if name.is_empty() {
                name = id.clone();
            }
            let signer = text(&d, "SignerIdentity");
            PhoneApp {
                sideloaded: signer.contains("Development") || signer.contains("Developer"),
                version: text(&d, "CFBundleShortVersionString"),
                bundle_id: id,
                name,
            }
        })
        .collect();
    out.sort_by(|a, b| b.sideloaded.cmp(&a.sideloaded).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

fn pairing_hint(msg: String) -> String {
    let l = msg.to_lowercase();
    if l.contains("pair") || l.contains("password") || l.contains("hostid") || l.contains("lockdown") {
        format!("{msg}\nUnlock the phone and tap \"Trust\" (enter the passcode), then try again.")
    } else {
        msg
    }
}

async fn remove(udid: &str, bundle: &str) -> Result<String, String> {
    let p = provider(udid).await?;
    let mut ip = idevice::installation_proxy::InstallationProxyClient::connect(&p)
        .await
        .map_err(|e| pairing_hint(format!("Couldn't reach the phone: {e}")))?;
    ip.uninstall(bundle.to_string(), None)
        .await
        .map_err(|e| format!("Couldn't remove {bundle}: {e}"))?;
    Ok(format!("Removed {bundle} from the phone."))
}

async fn dev_mode(udid: &str) -> Result<bool, String> {
    let p = provider(udid).await?;
    let mut amfi = idevice::amfi::AmfiClient::connect(&p)
        .await
        .map_err(|e| pairing_hint(format!("Couldn't check Developer Mode: {e}")))?;
    amfi.get_developer_mode_status()
        .await
        .map_err(|e| format!("Couldn't check Developer Mode: {e}"))
}

async fn reveal(udid: &str) -> Result<String, String> {
    let p = provider(udid).await?;
    let mut amfi = idevice::amfi::AmfiClient::connect(&p)
        .await
        .map_err(|e| pairing_hint(format!("Couldn't reach the phone: {e}")))?;
    amfi.reveal_developer_mode_option_in_ui()
        .await
        .map_err(|e| format!("Couldn't show the Developer Mode switch: {e}"))?;
    Ok("Done. On the phone: Settings › Privacy & Security › Developer Mode → On, then restart it.".into())
}

/// Runs one phone job in the background and reports back through `tx`.
pub fn spawn(udid: String, job: PhoneJob, tx: Sender<PhoneEvent>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(PhoneEvent::Done("Phone".into(), Err(e.to_string())));
                ctx.request_repaint();
                return;
            }
        };
        let event = rt.block_on(async {
            match job {
                PhoneJob::ListApps => PhoneEvent::Apps(list_apps(&udid).await),
                PhoneJob::Remove(b) => PhoneEvent::Done(format!("Remove {b}"), remove(&udid, &b).await),
                PhoneJob::DevModeStatus => PhoneEvent::DevMode(dev_mode(&udid).await),
                PhoneJob::RevealDevMode => PhoneEvent::Done("Developer Mode".into(), reveal(&udid).await),
            }
        });
        let _ = tx.send(event);
        ctx.request_repaint();
    });
}
