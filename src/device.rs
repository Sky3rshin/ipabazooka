//! Finds iPhones / iPads plugged into this computer by asking the Apple
//! Mobile Device service (usbmuxd) that iTunes / Apple Devices installs.

use std::io::{Read, Write};
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

fn connect() -> Result<TcpStream, String> {
    // On Windows the Apple Mobile Device service listens here.
    let addr: SocketAddr = "127.0.0.1:27015".parse().unwrap();
    let stream = TcpStream::connect_timeout(&addr, Duration::from_millis(600)).map_err(|_| {
        "Apple device service not found (install iTunes or Apple Devices)".to_string()
    })?;
    stream.set_read_timeout(Some(Duration::from_millis(1500))).ok();
    stream.set_write_timeout(Some(Duration::from_millis(1500))).ok();
    Ok(stream)
}

fn mux_send(stream: &mut TcpStream, dict: plist::Dictionary, tag: u32) -> Result<(), String> {
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

fn mux_recv(stream: &mut TcpStream) -> Result<plist::Dictionary, String> {
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
    let mut s = connect()?;
    let mut d = base_dict("Connect");
    d.insert("DeviceID".into(), plist::Value::Integer(device_id.into()));
    d.insert("PortNumber".into(), plist::Value::Integer((62078u16.to_be() as i64).into()));
    mux_send(&mut s, d, 2)?;
    let r = mux_recv(&mut s)?;
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
/// device service can't be reached.
pub fn list_devices() -> Result<Vec<Device>, String> {
    let mut s = connect()?;
    mux_send(&mut s, base_dict("ListDevices"), 1)?;
    let reply = mux_recv(&mut s)?;
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
