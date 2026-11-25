//! Finds iPhones / iPads plugged into this computer by asking usbmuxd: on
//! Windows that's the Apple Mobile Device service that iTunes / Apple Devices
//! installs, on Linux it's the usbmuxd daemon from the distribution.

use std::io::{Read, Write};
#[cfg(windows)]
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: i64,
    pub udid: String,
    pub name: String,
    pub connection: String,
    pub model: String,
    pub ios: String,
}

impl Device {
    pub fn label(&self) -> String {
        let mut s = self.name.clone();
        if !self.ios.is_empty() {
            s.push_str(&format!(" · iOS {}", self.ios));
        }
        s.push_str(&format!(" ({})", self.connection));
        s
    }
}

/// A connection to usbmuxd (TCP on Windows, a Unix socket on Linux).
trait MuxStream: Read + Write {}
impl<T: Read + Write> MuxStream for T {}

/// Why usbmuxd couldn't be reached.
enum Unreachable {
    /// Linux only: usbmuxd is installed but not running. Linux starts it when
    /// an iPhone or iPad is plugged in, so this simply means "no device".
    #[cfg_attr(windows, allow(dead_code))]
    Idle,
    /// It isn't installed, or can't be used; the text says what to do.
    Missing(String),
}

impl From<Unreachable> for String {
    fn from(u: Unreachable) -> String {
        match u {
            Unreachable::Idle => "No iPhone or iPad plugged in".into(),
            Unreachable::Missing(m) => m,
        }
    }
}

#[cfg(windows)]
fn connect() -> Result<Box<dyn MuxStream>, Unreachable> {
    // On Windows the Apple Mobile Device service listens here.
    let addr: SocketAddr = "127.0.0.1:27015".parse().unwrap();
    let stream = TcpStream::connect_timeout(&addr, Duration::from_millis(600)).map_err(|_| {
        Unreachable::Missing("Apple device service not found (install iTunes or Apple Devices)".to_string())
    })?;
    stream.set_read_timeout(Some(Duration::from_millis(1500))).ok();
    stream.set_write_timeout(Some(Duration::from_millis(1500))).ok();
    Ok(Box::new(stream))
}

/// Where Linux's usbmuxd listens (USBMUXD_SOCKET_ADDRESS can point elsewhere,
/// as "UNIX:/path/to/socket" or "host:port", like libimobiledevice allows).
#[cfg(not(windows))]
const USBMUXD_SOCKET: &str = "/var/run/usbmuxd";

#[cfg(not(windows))]
fn usbmuxd_installed() -> bool {
    [
        "/usr/sbin/usbmuxd",
        "/usr/bin/usbmuxd",
        "/sbin/usbmuxd",
        "/usr/local/sbin/usbmuxd",
        "/usr/local/bin/usbmuxd",
        "/run/current-system/sw/bin/usbmuxd",
    ]
    .iter()
    .any(|p| std::path::Path::new(p).exists())
}

#[cfg(not(windows))]
fn connect() -> Result<Box<dyn MuxStream>, Unreachable> {
    use std::os::unix::net::UnixStream;
    let timeouts = Some(Duration::from_millis(1500));
    if let Ok(addr) = std::env::var("USBMUXD_SOCKET_ADDRESS") {
        let addr = addr.trim();
        if let Some(path) = addr.strip_prefix("UNIX:") {
            let s = UnixStream::connect(path)
                .map_err(|e| Unreachable::Missing(format!("Couldn't reach usbmuxd at {path}: {e}")))?;
            s.set_read_timeout(timeouts).ok();
            s.set_write_timeout(timeouts).ok();
            return Ok(Box::new(s));
        }
        if let Ok(sock) = addr.parse::<std::net::SocketAddr>() {
            let s = std::net::TcpStream::connect_timeout(&sock, Duration::from_millis(600))
                .map_err(|e| Unreachable::Missing(format!("Couldn't reach usbmuxd at {addr}: {e}")))?;
            s.set_read_timeout(timeouts).ok();
            s.set_write_timeout(timeouts).ok();
            return Ok(Box::new(s));
        }
    }
    match UnixStream::connect(USBMUXD_SOCKET) {
        Ok(s) => {
            s.set_read_timeout(timeouts).ok();
            s.set_write_timeout(timeouts).ok();
            Ok(Box::new(s))
        }
        Err(e) => {
            use std::io::ErrorKind;
            let idle = matches!(e.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused);
            if idle && usbmuxd_installed() {
                Err(Unreachable::Idle)
            } else if idle {
                Err(Unreachable::Missing(
                    "usbmuxd (the service that lets Linux talk to an iPhone or iPad) isn't installed. \
                     Install it with your package manager, for example: sudo apt install usbmuxd"
                        .to_string(),
                ))
            } else {
                Err(Unreachable::Missing(format!("Couldn't reach usbmuxd ({USBMUXD_SOCKET}): {e}")))
            }
        }
    }
}

fn mux_send(stream: &mut dyn MuxStream, dict: plist::Dictionary, tag: u32) -> Result<(), String> {
    let mut body = Vec::new();
    plist::to_writer_xml(&mut body, &plist::Value::Dictionary(dict)).map_err(|e| e.to_string())?;
    let mut msg = Vec::with_capacity(16 + body.len());
    msg.extend_from_slice(&((16 + body.len()) as u32).to_le_bytes());
    msg.extend_from_slice(&1u32.to_le_bytes()); // version: plist
    msg.extend_from_slice(&8u32.to_le_bytes()); // message: plist
    msg.extend_from_slice(&tag.to_le_bytes());
    msg.extend_from_slice(&body);
    stream.write_all(&msg).map_err(|e| e.to_string())
}

fn mux_recv(stream: &mut dyn MuxStream) -> Result<plist::Dictionary, String> {
    let mut head = [0u8; 16];
    stream.read_exact(&mut head).map_err(|e| e.to_string())?;
    let len = u32::from_le_bytes([head[0], head[1], head[2], head[3]]) as usize;
    if !(16..=4 * 1024 * 1024).contains(&len) {
        return Err("bad reply size".into());
    }
    let mut body = vec![0u8; len - 16];
    stream.read_exact(&mut body).map_err(|e| e.to_string())?;
    let v = plist::Value::from_reader(std::io::Cursor::new(body)).map_err(|e| e.to_string())?;
    v.into_dictionary().ok_or_else(|| "reply was not a dictionary".into())
}

fn base_dict(kind: &str) -> plist::Dictionary {
    let mut d = plist::Dictionary::new();
    d.insert("MessageType".into(), kind.into());
    d.insert("ClientVersionString".into(), "ipabazooka-1.0".into());
    d.insert("ProgName".into(), "ipabazooka".into());
    d
}

/// Ask lockdownd (port 62078 on the device) for one public value.
fn lockdown_values(device_id: i64, keys: &[&str]) -> Result<Vec<Option<String>>, String> {
    let mut s = connect().map_err(String::from)?;
    let mut d = base_dict("Connect");
    d.insert("DeviceID".into(), plist::Value::Integer(device_id.into()));
    d.insert("PortNumber".into(), plist::Value::Integer((62078u16.to_be() as i64).into()));
    mux_send(&mut *s, d, 2)?;
    let r = mux_recv(&mut *s)?;
    let num = r.get("Number").and_then(|v| v.as_signed_integer()).unwrap_or(-1);
    if num != 0 {
        return Err(format!("device refused connection ({num})"));
    }
    let mut out = Vec::new();
    for key in keys {
        let mut q = plist::Dictionary::new();
        q.insert("Label".into(), "ipabazooka".into());
        q.insert("Request".into(), "GetValue".into());
        q.insert("Key".into(), (*key).into());
        let mut body = Vec::new();
        plist::to_writer_xml(&mut body, &plist::Value::Dictionary(q)).map_err(|e| e.to_string())?;
        s.write_all(&(body.len() as u32).to_be_bytes()).map_err(|e| e.to_string())?;
        s.write_all(&body).map_err(|e| e.to_string())?;
        let mut lb = [0u8; 4];
        s.read_exact(&mut lb).map_err(|e| e.to_string())?;
        let len = u32::from_be_bytes(lb) as usize;
        if len > 1024 * 1024 {
            return Err("bad reply size".into());
        }
        let mut rb = vec![0u8; len];
        s.read_exact(&mut rb).map_err(|e| e.to_string())?;
        let v = plist::Value::from_reader(std::io::Cursor::new(rb)).ok();
        let val = v
            .as_ref()
            .and_then(|v| v.as_dictionary())
            .and_then(|d| d.get("Value"))
            .and_then(|v| v.as_string())
            .map(|s| s.to_string());
        out.push(val);
    }
    Ok(out)
}

/// Lists connected devices. Returns an error message when the Apple
/// device service (usbmuxd) can't be reached.
pub fn list_devices() -> Result<Vec<Device>, String> {
    let mut s = match connect() {
        Ok(s) => s,
        // Linux runs usbmuxd only while a device is plugged in
        Err(Unreachable::Idle) => return Ok(Vec::new()),
        Err(Unreachable::Missing(m)) => return Err(m),
    };
    mux_send(&mut *s, base_dict("ListDevices"), 1)?;
    let reply = mux_recv(&mut *s)?;
    let list = reply
        .get("DeviceList")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut devices: Vec<Device> = Vec::new();
    for item in list {
        let Some(props) = item
            .as_dictionary()
            .and_then(|d| d.get("Properties"))
            .and_then(|p| p.as_dictionary())
        else {
            continue;
        };
        let id = props.get("DeviceID").and_then(|v| v.as_signed_integer()).unwrap_or(0);
        let udid = props
            .get("SerialNumber")
            .and_then(|v| v.as_string())
            .unwrap_or("")
            .to_string();
        let connection = props
            .get("ConnectionType")
            .and_then(|v| v.as_string())
            .unwrap_or("USB")
            .to_string();
        // Same phone can show up over USB and Wi-Fi; keep the first.
        if devices.iter().any(|d| d.udid == udid) {
            continue;
        }
        let mut dev = Device {
            id,
            udid,
            name: "iOS device".into(),
            connection: if connection == "Network" { "Wi-Fi".into() } else { connection },
            model: String::new(),
            ios: String::new(),
        };
        if let Ok(vals) = lockdown_values(id, &["DeviceName", "ProductType", "ProductVersion"]) {
            if let Some(Some(n)) = vals.first() {
                dev.name = n.clone();
            }
            if let Some(Some(m)) = vals.get(1) {
                dev.model = m.clone();
            }
            if let Some(Some(v)) = vals.get(2) {
                dev.ios = v.clone();
            }
        }
        devices.push(dev);
    }
    Ok(devices)
}
