// The config file, kept in the game's shared `config/` folder beside
// `options.txt` (Horizon API 26, `Game::native_config_dir`), so the window and
// its server thread read the same file.
//
// Injected into `crates/horizon-client/src/game.rs` as
// `pub(crate) mod tec_config`. The JSON itself is read and written by the
// dependency-free code in `tec_core`.

use crate::game::tec_core as core;
use crate::game::Game;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// The live config. One copy per process: the window, its integrated server
/// and a LAN host's server thread each load the same file.
static CONFIG: OnceLock<Mutex<core::Config>> = OnceLock::new();

pub(crate) fn path() -> Option<PathBuf> {
    Game::native_config_dir()
        .ok()
        .map(|dir| dir.join(core::CONFIG_FILE))
}

fn read_file(path: &PathBuf) -> Option<core::Config> {
    let text = std::fs::read_to_string(path).ok()?;
    match core::Config::from_json(&text) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            log::warn!("{}: {} is not usable ({e}); using the defaults", core::NAME, core::CONFIG_FILE);
            None
        }
    }
}

fn write_file(path: &PathBuf, cfg: &core::Config) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(path, cfg.to_json()) {
        log::warn!("{}: the config was not written: {e}", core::NAME);
    }
}

fn load() -> core::Config {
    let Some(path) = path() else {
        return core::Config::default();
    };
    match read_file(&path) {
        Some(cfg) => cfg,
        None => {
            let cfg = core::Config::default();
            // Write a commented-by-shape default so the player has something
            // to edit; never overwrite an existing file that failed to parse.
            if !path.exists() {
                write_file(&path, &cfg);
            }
            cfg
        }
    }
}

/// Logs the config's problems once, and again only when they change (a
/// reload may fix or add one). Called from every hook, so it stays cheap.
fn warn_once(cfg: &core::Config) {
    static WARNED: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    let lines = core::warnings(cfg);
    let cell = WARNED.get_or_init(|| Mutex::new(Vec::new()));
    let Ok(mut seen) = cell.lock() else { return };
    if *seen == lines {
        return;
    }
    for line in &lines {
        log::warn!("{}: {line}", core::NAME);
    }
    *seen = lines;
}

/// The config in force (loaded once; `reload` re-reads the file).
pub(crate) fn get() -> core::Config {
    let cell = CONFIG.get_or_init(|| Mutex::new(load()));
    let cfg = cell
        .lock()
        .map(|c| c.clone())
        .unwrap_or_else(|_| core::Config::default());
    warn_once(&cfg);
    cfg
}

/// Replaces the config and writes it back to the file.
pub(crate) fn set(cfg: &core::Config) {
    match CONFIG.get() {
        Some(cell) => {
            if let Ok(mut held) = cell.lock() {
                *held = cfg.clone();
            }
        }
        None => {
            let _ = CONFIG.set(Mutex::new(cfg.clone()));
        }
    }
    if let Some(path) = path() {
        write_file(&path, cfg);
    }
}

/// Re-reads the file (Options > Mods' "Reload config file").
pub(crate) fn reload() -> core::Config {
    let cfg = load();
    match CONFIG.get() {
        Some(cell) => {
            if let Ok(mut held) = cell.lock() {
                *held = cfg.clone();
            }
        }
        None => {
            let _ = CONFIG.set(Mutex::new(cfg.clone()));
        }
    }
    cfg
}

// ---------------------------------------------------------------- settings page

/// The Options > Mods page's rows for this mod.
pub(crate) fn settings_buttons() -> Vec<String> {
    core::settings_labels(&get())
}

/// A click on one of those rows.
pub(crate) fn settings_click(button: usize) {
    let Some(action) = core::settings_action(button) else {
        return;
    };
    match action {
        core::SettingsAction::Reload => {
            reload();
        }
        _ => {
            let next = core::apply_settings(&get(), action);
            set(&next);
        }
    }
}
