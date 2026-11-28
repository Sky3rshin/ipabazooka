//! Builds the small 32-bit "local anisette" helper (helper/anisette_helper.cs)
//! with the C# compiler that ships with Windows (.NET Framework 4), so the app
//! can ask this PC's iCloud for Windows for Apple sign-in data. The helper's
//! bytes are embedded into the app. If the compiler isn't there (or this isn't
//! Windows), an empty placeholder is written and the feature is simply off.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=helper/anisette_helper.cs");
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let exe = out.join("anisette-helper.exe");

    let windows = std::env::var("CARGO_CFG_WINDOWS").is_ok();
    let built = windows && compile(&exe);
    if !built {
        let _ = std::fs::write(&exe, b"");
        // Only worth a warning on Windows; other systems never use the helper.
        if windows {
            println!("cargo:warning=local anisette helper not built (needs Windows' .NET Framework C# compiler); online sign-in servers will be used");
        }
    }

    // Give ipabazooka.exe its own icon and version details in File Explorer.
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
        if !embed_resources(&out) {
            println!("cargo:warning=exe icon/version info not embedded (windres not found); the app still works");
        }
    }
}

/// Draws the app icon (a rounded square with a download arrow) at `n`×`n`
/// pixels, RGBA. Same design as the window icon, scaled.
fn draw_icon(n: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; n * n * 4];
    let k = 64.0 / n as f32; // work in a 64-unit design space
    let r = 14.0f32;
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = ((x as f32 + 0.5) * k, (y as f32 + 0.5) * k);
            let cx = fx.clamp(r, 64.0 - r);
            let cy = fy.clamp(r, 64.0 - r);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let a = ((r + 0.5 * k - d) / k).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            let t = fy / 64.0;
            let mut c = [0u8, (200.0 - 60.0 * t) as u8, (60.0 - 20.0 * t) as u8];
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
    rgba
}

/// Writes a Windows .ico with several sizes (32-bit BMP entries).
fn write_ico(path: &std::path::Path) -> std::io::Result<()> {
    let sizes = [16usize, 24, 32, 48, 64, 128, 256];
    let mut images: Vec<Vec<u8>> = Vec::new();
    for &n in &sizes {
        let rgba = draw_icon(n);
        let mut b = Vec::new();
        // BITMAPINFOHEADER (height doubled: colour rows + AND mask rows)
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(&(n as i32).to_le_bytes());
        b.extend_from_slice(&((n * 2) as i32).to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&32u16.to_le_bytes());
        b.extend_from_slice(&[0u8; 24]);
        // pixels, bottom-up, BGRA
        for y in (0..n).rev() {
            for x in 0..n {
                let i = (y * n + x) * 4;
                b.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
            }
        }
        // AND mask: all zero (alpha does the work), rows padded to 4 bytes
        let row = ((n + 31) / 32) * 4;
        b.extend(std::iter::repeat(0u8).take(row * n));
        images.push(b);
    }
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len();
    for (i, &n) in sizes.iter().enumerate() {
        let dim = if n >= 256 { 0u8 } else { n as u8 };
        ico.extend_from_slice(&[dim, dim, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(images[i].len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += images[i].len();
    }
    for img in &images {
        ico.extend_from_slice(img);
    }
    std::fs::write(path, ico)
}

/// Builds icon + version resources with windres (MinGW) and links them in.
fn embed_resources(out: &std::path::Path) -> bool {
    let ico = out.join("ipabazooka.ico");
    if write_ico(&ico).is_err() {
        return false;
    }
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "1.0.0".into());
    let mut parts: Vec<u16> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    parts.resize(4, 0);
    let commas = parts.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
    let rc = format!(
        "1 ICON \"{ico}\"\n\
         1 VERSIONINFO\n\
         FILEVERSION {commas}\n\
         PRODUCTVERSION {commas}\n\
         BEGIN\n\
           BLOCK \"StringFileInfo\"\n\
           BEGIN\n\
             BLOCK \"040904B0\"\n\
             BEGIN\n\
               VALUE \"FileDescription\", \"ipabazooka - install apps on your iPhone\"\n\
               VALUE \"ProductName\", \"ipabazooka\"\n\
               VALUE \"FileVersion\", \"{version}\"\n\
               VALUE \"ProductVersion\", \"{version}\"\n\
               VALUE \"OriginalFilename\", \"ipabazooka.exe\"\n\
             END\n\
           END\n\
           BLOCK \"VarFileInfo\"\n\
           BEGIN\n\
             VALUE \"Translation\", 0x409, 1200\n\
           END\n\
         END\n",
        ico = ico.display().to_string().replace('\\', "/"),
    );
    let rc_path = out.join("ipabazooka.rc");
    let obj = out.join("ipabazooka_res.o");
    if std::fs::write(&rc_path, rc).is_err() {
        return false;
    }
    for tool in ["windres", "x86_64-w64-mingw32-windres"] {
        let ok = Command::new(tool)
            .arg("--input")
            .arg(&rc_path)
            .arg("--output")
            .arg(&obj)
            .arg("--output-format=coff")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok && obj.exists() {
            println!("cargo:rustc-link-arg-bins={}", obj.display());
            return true;
        }
    }
    false
}

fn compile(exe: &PathBuf) -> bool {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let candidates = [
        format!(r"{windir}\Microsoft.NET\Framework\v4.0.30319\csc.exe"),
        format!(r"{windir}\Microsoft.NET\Framework64\v4.0.30319\csc.exe"),
    ];
    let src = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("helper").join("anisette_helper.cs");
    for csc in candidates.iter().filter(|c| std::path::Path::new(c).exists()) {
        let status = Command::new(csc)
            .arg("/nologo")
            .arg("/platform:x86")
            .arg("/optimize+")
            .arg("/target:exe")
            .arg(format!("/out:{}", exe.display()))
            .arg(&src)
            .status();
        if matches!(status, Ok(s) if s.success()) && exe.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            return true;
        }
    }
    false
}
