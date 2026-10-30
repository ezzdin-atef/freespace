//! App logos for the installed-apps list, as `data:image/png;base64,...` URLs the web UI can show directly.
//!
//! Desktop apps: the icon their uninstall entry points at (DisplayIcon), else the uninstaller's own icon, else
//! the main .exe in the install folder, extracted at `SIZE` px. Store apps: the tile logo in AppxManifest.xml.

use crate::apps::{InstalledApp, Source};
use crate::util::wide;
use base64::Engine;
use std::fs;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, PrivateExtractIconsW, HICON, ICONINFO};

/// Extracted size in px: twice the 24px the list shows, so logos stay sharp at 200% scaling.
const SIZE: i32 = 48;

/// The app's logo as a PNG data URL, or None when it has none we can read.
pub fn app_logo(app: &InstalledApp) -> Option<String> {
    match &app.source {
        Source::Store { .. } => store_logo(app.location.as_deref()?),
        Source::Registry { uninstall, .. } => app
            .icon
            .as_deref()
            .and_then(|i| icon_from_location(i))
            .or_else(|| uninstall.as_deref().and_then(icon_of_uninstaller))
            .or_else(|| main_exe(&app.name, app.location.as_deref()?).and_then(|e| extract(&e, 0))),
    }
}

fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

/// "%ProgramFiles%\App\app.exe" -> "C:\Program Files\App\app.exe"
fn expand_env(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let src = wide(s);
    let mut buf = vec![0u16; 1024];
    let n = unsafe { ExpandEnvironmentStringsW(src.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if n == 0 || n > buf.len() {
        return s.to_string();
    }
    String::from_utf16_lossy(&buf[..n - 1])
}

/// Splits a DisplayIcon value like `"C:\App\app.exe",-101` into (path, index).
fn parse_location(value: &str) -> (PathBuf, i32) {
    let v = value.trim();
    let (path, index) = match v.rsplit_once(',') {
        Some((p, i)) if i.trim().parse::<i32>().is_ok() => (p, i.trim().parse().unwrap_or(0)),
        _ => (v, 0),
    };
    (PathBuf::from(expand_env(path.trim().trim_matches('"'))), index)
}

fn icon_from_location(value: &str) -> Option<String> {
    let (path, index) = parse_location(value);
    if !path.is_file() {
        return None;
    }
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
        return fs::read(&path).ok().map(|b| data_url(&b));
    }
    extract(&path, index)
}

/// The uninstaller often carries the app's icon (Inno Setup, NSIS); msiexec and rundll32 do not.
fn icon_of_uninstaller(cmd: &str) -> Option<String> {
    let cmd = cmd.trim();
    let exe = match cmd.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => &cmd[..cmd.to_lowercase().find(".exe")? + 4],
    };
    let path = PathBuf::from(expand_env(exe));
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name == "msiexec.exe" || name == "rundll32.exe" || !path.is_file() {
        return None;
    }
    extract(&path, 0)
}

/// The app's own .exe in its install folder (or one level down): one whose name matches the app's, else
/// the largest. Installers, uninstallers, updaters and crash reporters are skipped.
fn main_exe(app_name: &str, dir: &Path) -> Option<PathBuf> {
    const SKIP: [&str; 7] = ["unins", "uninst", "setup", "install", "update", "crash", "helper"];
    let words: Vec<String> = app_name
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(str::to_string)
        .collect();
    let mut exes = Vec::new();
    let mut collect = |d: &Path| {
        for e in fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            let is_exe = p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe"));
            if is_exe && !SKIP.iter().any(|s| stem.contains(s)) {
                let size = e.metadata().map_or(0, |m| m.len());
                let named = words.iter().any(|w| stem.contains(w.as_str()));
                exes.push((named, size, p));
            }
        }
    };
    collect(dir);
    for sub in fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.path().is_dir()).take(20) {
        collect(&sub.path());
    }
    exes.into_iter().max_by_key(|(named, size, _)| (*named, *size)).map(|(_, _, p)| p)
}

fn extract(path: &Path, index: i32) -> Option<String> {
    let w = wide(&path.to_string_lossy());
    let mut icon: HICON = std::ptr::null_mut();
    let n = unsafe { PrivateExtractIconsW(w.as_ptr(), index, SIZE, SIZE, &mut icon, std::ptr::null_mut(), 1, 0) };
    if n == 0 || n == u32::MAX || icon.is_null() {
        return None;
    }
    let rgba = unsafe { icon_rgba(icon) };
    unsafe { DestroyIcon(icon) };
    encode_png(&rgba?)
}

/// The icon's pixels as RGBA, `SIZE` x `SIZE`, top row first.
unsafe fn icon_rgba(icon: HICON) -> Option<Vec<u8>> {
    let mut info: ICONINFO = std::mem::zeroed();
    if GetIconInfo(icon, &mut info) == 0 {
        return None;
    }
    let dc = CreateCompatibleDC(std::ptr::null_mut());
    let read = |bitmap| {
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: SIZE,
            biHeight: -SIZE, // negative: top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut buf = vec![0u8; (SIZE * SIZE * 4) as usize];
        let lines = GetDIBits(dc, bitmap, 0, SIZE as u32, buf.as_mut_ptr().cast(), &mut bmi, DIB_RGB_COLORS);
        (lines == SIZE).then_some(buf)
    };
    let color = (!info.hbmColor.is_null()).then(|| read(info.hbmColor)).flatten();
    // Old icons keep transparency in a separate 1-bit mask instead of an alpha channel.
    let mask = color
        .as_ref()
        .filter(|c| c.chunks_exact(4).all(|p| p[3] == 0))
        .and_then(|_| (!info.hbmMask.is_null()).then(|| read(info.hbmMask)).flatten());
    DeleteDC(dc);
    if !info.hbmColor.is_null() {
        DeleteObject(info.hbmColor);
    }
    if !info.hbmMask.is_null() {
        DeleteObject(info.hbmMask);
    }

    let mut px = color?;
    for (i, p) in px.chunks_exact_mut(4).enumerate() {
        p.swap(0, 2); // BGRA -> RGBA
        if let Some(m) = &mask {
            p[3] = if m[i * 4] == 0 { 255 } else { 0 };
        }
    }
    px.chunks_exact(4).any(|p| p[3] != 0).then_some(px)
}

fn encode_png(rgba: &[u8]) -> Option<String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, SIZE as u32, SIZE as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(rgba).ok()?;
    }
    Some(data_url(&out))
}

/// Value of `name="..."` in the manifest, or the text of `<name>...</name>`.
fn manifest_value(xml: &str, name: &str) -> Option<String> {
    // Leading space, so "Logo" does not match inside "Square44x44Logo".
    let attr = format!(" {name}=\"");
    if let Some(i) = xml.find(&attr) {
        let rest = &xml[i + attr.len()..];
        return rest.split('"').next().map(str::to_string);
    }
    let open = format!("<{name}>");
    let i = xml.find(&open)? + open.len();
    xml[i..].split('<').next().map(|s| s.trim().to_string())
}

/// The Store tile logo. Manifests name a base file ("Assets\Square44x44Logo.png") that exists on disk only
/// as variants like "Square44x44Logo.targetsize-48_altform-unplated.png"; pick the best of those.
fn store_logo(dir: &Path) -> Option<String> {
    let xml = fs::read_to_string(dir.join("AppxManifest.xml")).ok()?;
    ["Square44x44Logo", "Square150x150Logo", "Logo"]
        .iter()
        .filter_map(|n| manifest_value(&xml, n))
        .find_map(|rel| best_variant(&dir.join(rel.replace('/', "\\"))))
        .and_then(|p| fs::read(p).ok())
        .map(|b| data_url(&b))
}

fn best_variant(base: &Path) -> Option<PathBuf> {
    if base.is_file() {
        return Some(base.to_path_buf());
    }
    let stem = base.file_stem()?.to_string_lossy().to_lowercase();
    let score = |name: &str| {
        let n = name.to_lowercase();
        let unplated = n.contains("unplated") as i32 * 10;
        let size = if n.contains("targetsize-48") {
            8
        } else if n.contains("targetsize-32") || n.contains("scale-200") {
            6
        } else if n.contains("scale-100") || n.contains("targetsize-24") {
            4
        } else {
            1
        };
        // Light/dark-theme variants are tuned for one background only.
        let themed = (n.contains("contrast-") || n.contains("theme-")) as i32 * -20;
        unplated + size + themed
    };
    fs::read_dir(base.parent()?)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let l = n.to_lowercase();
            l.starts_with(&format!("{stem}.")) && l.ends_with(".png")
        })
        .max_by_key(|n| score(n))
        .map(|n| base.with_file_name(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_icon_values_parse() {
        let (p, i) = parse_location(r#""C:\Program Files\App\app.exe",-101"#);
        assert_eq!((p, i), (PathBuf::from(r"C:\Program Files\App\app.exe"), -101));
        let (p, i) = parse_location(r"C:\App\app.ico");
        assert_eq!((p, i), (PathBuf::from(r"C:\App\app.ico"), 0));
        let (p, _) = parse_location(r"%SystemRoot%\system32\notepad.exe,0");
        assert!(p.is_file(), "{p:?}");
    }

    #[test]
    fn manifest_values() {
        let xml = r#"<Properties><Logo>Assets\StoreLogo.png</Logo></Properties><uap:VisualElements Square44x44Logo="Assets\Square44.png" />"#;
        assert_eq!(manifest_value(xml, "Square44x44Logo").as_deref(), Some(r"Assets\Square44.png"));
        assert_eq!(manifest_value(xml, "Logo").as_deref(), Some(r"Assets\StoreLogo.png"));
    }

    #[test]
    fn extracts_a_real_icon() {
        let url = icon_from_location(r"%SystemRoot%\explorer.exe,0").expect("explorer has an icon");
        assert!(url.starts_with("data:image/png;base64,") && url.len() > 200);
    }
}

#[cfg(test)]
mod coverage {
    /// How many installed apps get a logo, and how long reading them takes:
    /// cargo test --release -- --ignored --nocapture logo_coverage
    #[test]
    #[ignore]
    fn logo_coverage() {
        use rayon::prelude::*;
        let mut apps = crate::apps::list_registry();
        apps.extend(crate::apps::list_store());
        let t = std::time::Instant::now();
        let logos: Vec<Option<String>> = apps.par_iter().map(super::app_logo).collect();
        let found = logos.iter().filter(|l| l.is_some()).count();
        let bytes: usize = logos.iter().flatten().map(String::len).sum();
        println!("{found}/{} apps have a logo, {} KB, {:?}", apps.len(), bytes / 1024, t.elapsed());
        for (a, l) in apps.iter().zip(&logos).filter(|(_, l)| l.is_none()).take(25) {
            println!("  none: {} ({}) icon={:?}", a.name, if a.is_store() { "store" } else { "desktop" }, a.icon);
            let _ = l;
        }
    }
}
