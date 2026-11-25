//! Reads the basic facts out of an .ipa app file (a zip with
//! Payload/<Name>.app/Info.plist inside).

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct IpaInfo {
    pub path: PathBuf,
    pub file_name: String,
    pub name: String,
    pub bundle_id: String,
    pub version: String,
    pub build: String,
    pub min_ios: String,
    pub size_bytes: u64,
    pub file_count: usize,
    pub extensions: usize,
}

pub fn read_ipa(path: &Path) -> Result<IpaInfo, String> {
    let file = File::open(path).map_err(|e| format!("Can't open file: {e}"))?;
    let size_bytes = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "This isn't a valid .ipa (not a zip archive)".to_string())?;

    let mut plist_name: Option<String> = None;
    let mut extensions = 0usize;
    for i in 0..zip.len() {
        let Some(raw) = zip.name_for_index(i) else { continue };
        // Some Windows zip tools write backslashes; treat them like slashes.
        let norm = raw.replace('\\', "/");
        let parts: Vec<&str> = norm.split('/').collect();
        if parts.len() == 3
            && parts[0] == "Payload"
            && parts[1].ends_with(".app")
            && parts[2] == "Info.plist"
        {
            plist_name = Some(raw.to_string());
        }
        if parts.len() == 5
            && parts[0] == "Payload"
            && parts[2] == "PlugIns"
            && parts[3].ends_with(".appex")
            && parts[4] == "Info.plist"
        {
            extensions += 1;
        }
    }
    let plist_name = plist_name.ok_or("No app found inside (missing Payload/*.app/Info.plist)")?;
    let mut data = Vec::new();
    zip.by_name(&plist_name)
        .map_err(|e| e.to_string())?
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    let value = plist::Value::from_reader(std::io::Cursor::new(data))
        .map_err(|_| "The app's Info.plist couldn't be read".to_string())?;
    let dict = value.as_dictionary().ok_or("The app's Info.plist is not a dictionary")?;
    let get = |k: &str| dict.get(k).and_then(|v| v.as_string()).unwrap_or("").to_string();

    let app_folder = plist_name.replace('\\', "/").split('/').nth(1).unwrap_or("").trim_end_matches(".app").to_string();
    let mut name = get("CFBundleDisplayName");
    if name.is_empty() {
        name = get("CFBundleName");
    }
    if name.is_empty() {
        name = app_folder;
    }

    Ok(IpaInfo {
        path: path.to_path_buf(),
        file_name: path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        name,
        bundle_id: get("CFBundleIdentifier"),
        version: get("CFBundleShortVersionString"),
        build: get("CFBundleVersion"),
        min_ios: get("MinimumOSVersion"),
        size_bytes,
        file_count: zip.len(),
        extensions,
    })
}

pub fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} GB", b / (1024.0 * 1024.0 * 1024.0))
    } else if b >= 1024.0 * 1024.0 {
        format!("{:.1} MB", b / (1024.0 * 1024.0))
    } else {
        format!("{:.0} KB", b / 1024.0)
    }
}
