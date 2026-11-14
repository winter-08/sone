//! The `tidal://` link handler registration Tauri's deep-link plugin did at every launch on
//! Linux (`register_all` for the configured scheme): a `<binary>-handler.desktop` entry in
//! `$XDG_DATA_HOME/applications` naming this binary (or the AppImage) for
//! `x-scheme-handler/tidal`, then `update-desktop-database` and `xdg-mime default`. An entry
//! already there keeps its other fields and gains the mime type or the new Exec only if they
//! changed, as the plugin's ini edit did.
//!
//! Release builds only: a debug build registering at every launch pointed the system's handler
//! at whichever build ran last. Not run under the verifier's sandbox (`SPIKE_INNER`) or with
//! `SONE_NO_SCHEME_REGISTER` set either; tests call [`write_entry`] on a temporary directory.

use std::path::{Path, PathBuf};

const SCHEME: &str = "tidal";

/// At start, off the UI thread: register as Tauri did, logging (not failing) on errors.
pub fn register_at_start() {
    if !registers(cfg!(target_os = "linux"), cfg!(debug_assertions), |k| std::env::var_os(k).is_some()) {
        return;
    }
    let _ = std::thread::Builder::new().name("sone-scheme".into()).spawn(|| {
        if let Err(e) = register() {
            log::warn!("[rune] tidal:// handler registration failed: {e}");
        }
    });
}

/// Whether this launch registers the handler: a Linux release build, outside the verifier's
/// sandbox, without `SONE_NO_SCHEME_REGISTER`.
fn registers(linux: bool, debug: bool, set: impl Fn(&str) -> bool) -> bool {
    linux && !debug && !set("SPIKE_INNER") && !set("SONE_NO_SCHEME_REGISTER")
}

fn register() -> std::io::Result<()> {
    let exe = launcher(&std::env::current_exe()?);
    let exec = std::env::var_os("APPIMAGE").map(PathBuf::from).unwrap_or(exe.clone());
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .ok_or_else(|| std::io::Error::other("no data directory"))?;
    let dir = data.join("applications");
    let name = exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "sone-rune".into());
    let file = write_entry(&dir, &name, &exec, crate::TITLE)?;
    let file_name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    std::process::Command::new("update-desktop-database").arg(&dir).status()?;
    std::process::Command::new("xdg-mime").args(["default", &file_name, &format!("x-scheme-handler/{SCHEME}")]).status()?;
    Ok(())
}

/// The program a link should start: this binary, or for a Nix-wrapped one
/// (`.sone-rune-wrapped`) the wrapper beside it, which sets up its environment.
fn launcher(exe: &Path) -> PathBuf {
    let name = exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(inner) = name.strip_prefix('.').and_then(|n| n.strip_suffix("-wrapped")) {
        let outer = exe.with_file_name(inner);
        if outer.exists() {
            return outer;
        }
    }
    exe.to_path_buf()
}

/// Writes (or updates) `<bin>-handler.desktop` in `dir` for `exec`; returns its path.
pub fn write_entry(dir: &Path, bin: &str, exec: &Path, app_name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{bin}-handler.desktop"));
    let mime = format!("x-scheme-handler/{SCHEME}");
    let qualified_exec = format!("\"{}\" %u", exec.display());
    match std::fs::read_to_string(&path) {
        Ok(old) => {
            if let Some(new) = updated_entry(&old, &mime, &qualified_exec) {
                std::fs::write(&path, new)?;
            }
        }
        Err(_) => std::fs::write(&path, format!("[Desktop Entry]\nType=Application\nName={app_name}\nExec={qualified_exec}\nTerminal=false\nMimeType={mime}\nNoDisplay=true\n"))?,
    }
    Ok(path)
}

/// An existing entry with `mime` added to its MimeType and Exec set to `exec`, or None when it
/// already has both (the plugin rewrote the file only on a change).
fn updated_entry(old: &str, mime: &str, exec: &str) -> Option<String> {
    let mut changed = false;
    let (mut saw_mime, mut saw_exec) = (false, false);
    let mut out: Vec<String> = Vec::new();
    for line in old.lines() {
        if let Some(v) = line.strip_prefix("MimeType=") {
            saw_mime = true;
            if v.split(';').any(|m| m == mime) {
                out.push(line.to_string());
            } else {
                changed = true;
                out.push(format!("MimeType={mime};{v}"));
            }
        } else if let Some(v) = line.strip_prefix("Exec=") {
            saw_exec = true;
            if v == exec {
                out.push(line.to_string());
            } else {
                changed = true;
                out.push(format!("Exec={exec}"));
            }
        } else {
            out.push(line.to_string());
        }
    }
    if !saw_mime {
        changed = true;
        out.push(format!("MimeType={mime};"));
    }
    if !saw_exec {
        changed = true;
        out.push(format!("Exec={exec}"));
    }
    changed.then(|| out.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_linux_release_build_registers() {
        let none = |_: &str| false;
        assert!(registers(true, false, none));
        assert!(!registers(true, true, none));
        assert!(!registers(false, false, none));
        assert!(!registers(true, false, |k| k == "SPIKE_INNER"));
        assert!(!registers(true, false, |k| k == "SONE_NO_SCHEME_REGISTER"));
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sone-scheme-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn writes_the_plugins_entry() {
        let dir = temp("new");
        let p = write_entry(&dir, "sone-rune", Path::new("/opt/sone/sone-rune"), "SONE").unwrap();
        assert_eq!(p.file_name().unwrap(), "sone-rune-handler.desktop");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "[Desktop Entry]\nType=Application\nName=SONE\nExec=\"/opt/sone/sone-rune\" %u\nTerminal=false\nMimeType=x-scheme-handler/tidal\nNoDisplay=true\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn updates_an_entry_only_where_it_changed() {
        let dir = temp("update");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("sone-rune-handler.desktop");
        std::fs::write(&p, "[Desktop Entry]\nName=Mine\nExec=\"/old\" %u\nMimeType=x-scheme-handler/foo;\n").unwrap();
        write_entry(&dir, "sone-rune", Path::new("/new"), "SONE").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "[Desktop Entry]\nName=Mine\nExec=\"/new\" %u\nMimeType=x-scheme-handler/tidal;x-scheme-handler/foo;\n");
        // nothing to change: the file is left as it is
        assert_eq!(updated_entry(&std::fs::read_to_string(&p).unwrap(), "x-scheme-handler/tidal", "\"/new\" %u"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_nix_wrapped_binary_registers_its_wrapper() {
        let dir = temp("wrap");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("sone-rune"), "").unwrap();
        assert_eq!(launcher(&dir.join(".sone-rune-wrapped")), dir.join("sone-rune"));
        assert_eq!(launcher(&dir.join("other")), dir.join("other"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
