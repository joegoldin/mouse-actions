use mouse_actions;
use mouse_actions::config;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::Manager;

/// The systemd user unit we expect to manage the gesture daemon. Hardcoded
/// to match what the home-manager module installs (see
/// `hosts/common/home/mouse-actions/default.nix` in the consumer dotfiles).
const SERVICE_UNIT: &str = "mouse-actions.service";

#[derive(Serialize, Clone, Debug)]
pub struct ServiceStatus {
    /// `LoadState=loaded` — the unit file is installed and known to systemd.
    /// When false, every other field is meaningless and the GUI falls back
    /// to managing the daemon as a direct subprocess.
    available: bool,
    /// `ActiveState=active`.
    active: bool,
    /// "active" | "inactive" | "failed" | "activating" | "deactivating" | ...
    active_state: String,
    /// "running" | "dead" | "exited" | "failed" | "auto-restart" | ...
    sub_state: String,
    /// Absolute path the loaded unit was read from. Empty if the unit isn't
    /// loaded. Used to distinguish a GUI-installed unit (under the user's
    /// XDG config directory) from one installed by the OS/distro.
    fragment_path: String,
    /// True when the loaded unit lives under `$XDG_CONFIG_HOME/systemd/user/`
    /// — i.e. it was either installed by this GUI or hand-placed by the user.
    /// The Uninstall affordance is gated on this so we never delete an
    /// OS-managed unit.
    user_installed: bool,
}

fn systemctl_show(prop: &str) -> Option<String> {
    let out = Command::new("systemctl")
        .args(["--user", "show", "-p", prop, "--value", SERVICE_UNIT])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[tauri::command(async)]
fn service_status() -> ServiceStatus {
    let load = systemctl_show("LoadState").unwrap_or_default();
    let active_state = systemctl_show("ActiveState").unwrap_or_default();
    let sub_state = systemctl_show("SubState").unwrap_or_default();
    let fragment_path = systemctl_show("FragmentPath").unwrap_or_default();
    let available = load == "loaded";
    let user_installed = available && {
        let expected = user_unit_path();
        let expected_str = expected.to_string_lossy();
        fragment_path == expected_str
            || (expected.parent().is_some_and(|p| {
                fragment_path.starts_with(&*p.to_string_lossy())
            }) && !fragment_path.is_empty())
    };
    ServiceStatus {
        available,
        active: active_state == "active",
        active_state,
        sub_state,
        fragment_path,
        user_installed,
    }
}

fn systemctl_action(action: &str) -> Result<(), String> {
    let out = Command::new("systemctl")
        .args(["--user", action, SERVICE_UNIT])
        .output()
        .map_err(|e| format!("spawn systemctl: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("systemctl --user {} {}: exit {:?}", action, SERVICE_UNIT, out.status.code())
        } else {
            err
        });
    }
    Ok(())
}

#[tauri::command(async)]
fn service_start() -> Result<(), String> {
    systemctl_action("start")
}

#[tauri::command(async)]
fn service_stop() -> Result<(), String> {
    systemctl_action("stop")
}

#[tauri::command(async)]
fn service_restart() -> Result<(), String> {
    systemctl_action("restart")
}

/// `$XDG_CONFIG_HOME/systemd/user/mouse-actions.service`.
fn user_unit_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let mut h = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
            h.push(".config");
            h
        });
    base.join("systemd").join("user").join(SERVICE_UNIT)
}

/// Locate the `mouse-actions` CLI binary, used as the unit's `ExecStart`.
/// Prefer a sibling of the running GUI binary (same install dir); fall back
/// to PATH lookup. Returns the absolute resolved path.
fn locate_daemon_binary() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe_canon = std::fs::canonicalize(&exe).unwrap_or(exe);
    if let Some(dir) = exe_canon.parent() {
        let sibling = dir.join("mouse-actions");
        if sibling.is_file() {
            return std::fs::canonicalize(&sibling).map_err(|e| e.to_string());
        }
    }
    let which = Command::new("sh")
        .arg("-c")
        .arg("command -v mouse-actions")
        .output()
        .map_err(|e| format!("which mouse-actions: {e}"))?;
    if which.status.success() {
        let path = String::from_utf8_lossy(&which.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    Err("Could not locate the `mouse-actions` CLI binary — install it next to mouse-actions-gui or put it on PATH.".to_string())
}

fn unit_file_contents(daemon: &Path) -> String {
    format!(
        "# Generated by mouse-actions-gui. Safe to remove via the Uninstall\n\
         # button in the GUI, or by hand:\n\
         #   systemctl --user disable --now mouse-actions.service\n\
         #   rm {unit}\n\
         #   systemctl --user daemon-reload\n\
         [Unit]\n\
         Description=mouse-actions gesture daemon\n\
         PartOf=graphical-session.target\n\
         After=graphical-session.target\n\
         \n\
         [Service]\n\
         ExecStart={daemon} start\n\
         Restart=on-failure\n\
         RestartSec=3\n\
         \n\
         [Install]\n\
         WantedBy=graphical-session.target\n",
        unit = user_unit_path().display(),
        daemon = daemon.display(),
    )
}

#[tauri::command(async)]
fn service_install() -> Result<(), String> {
    let daemon = locate_daemon_binary()?;
    let unit_path = user_unit_path();
    if let Some(parent) = unit_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    std::fs::write(&unit_path, unit_file_contents(&daemon))
        .map_err(|e| format!("write {}: {e}", unit_path.display()))?;
    let reload = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()
        .map_err(|e| format!("systemctl daemon-reload: {e}"))?;
    if !reload.success() {
        return Err(format!("daemon-reload exited {:?}", reload.code()));
    }
    let enable = Command::new("systemctl")
        .args(["--user", "enable", "--now", SERVICE_UNIT])
        .output()
        .map_err(|e| format!("systemctl enable --now: {e}"))?;
    if !enable.status.success() {
        let err = String::from_utf8_lossy(&enable.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("enable --now exited {:?}", enable.status.code())
        } else {
            err
        });
    }
    Ok(())
}

#[tauri::command(async)]
fn service_uninstall() -> Result<(), String> {
    let status = service_status();
    if !status.user_installed {
        return Err(format!(
            "Refusing to uninstall: the loaded unit at {} wasn't installed under \
             $XDG_CONFIG_HOME/systemd/user/ (this is probably an OS-managed unit).",
            if status.fragment_path.is_empty() {
                "<no unit loaded>"
            } else {
                &status.fragment_path
            }
        ));
    }
    // Best-effort stop/disable — the unit file removal below is what actually
    // takes the service away from systemd's view.
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", SERVICE_UNIT])
        .status();
    let unit_path = user_unit_path();
    if unit_path.exists() {
        std::fs::remove_file(&unit_path)
            .map_err(|e| format!("rm {}: {e}", unit_path.display()))?;
    }
    let reload = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()
        .map_err(|e| format!("systemctl daemon-reload: {e}"))?;
    if !reload.success() {
        return Err(format!("daemon-reload exited {:?}", reload.code()));
    }
    Ok(())
}

#[tauri::command]
fn get_default_config_path() -> String {
    format!(
        "get_default_config_path={:?}",
        mouse_actions::config::get_config_path(&None)
    )
}

#[tauri::command]
fn get_version() -> String {
    format!("v{}", mouse_actions::process_args::get_version())
}

#[tauri::command(async)]
fn stop() {
    let ma_exe_path = std::env::current_exe().unwrap();
    mouse_actions::process_event::process_cmd(vec![
        ma_exe_path.to_str().unwrap().to_string(),
        String::from("stop"),
    ])
}

#[tauri::command(async)]
fn start() {
    let ma_exe_path = std::env::current_exe().unwrap();
    let args = mouse_actions::args::parse();
    let mut cmd: Vec<String> = Vec::new();
    cmd.push(ma_exe_path.to_str().unwrap().to_string());
    // FIXME : generic forward args
    if args.no_listen {
        cmd.push(String::from("--no-listen"));
    }
    if args.config_path.is_some() {
        cmd.push(String::from("--config-path"));
        cmd.push(args.config_path.unwrap());
    }
    if args.log_level.is_some() {
        cmd.push(String::from("--log-level"));
        cmd.push(args.log_level.unwrap());
    }
    cmd.push(String::from("start"));
    mouse_actions::process_event::process_cmd(cmd)
}

#[tauri::command(async)]
fn get_config() -> config::Config {
    let args = mouse_actions::args::parse();
    let config_path = config::get_config_path(&args.config_path);
    config::init_config_file_if_not_exists(&config_path);
    config::get_config(&config_path)
}

#[tauri::command(async)]
fn save_config(new_config: config::Config) {
    let args = mouse_actions::args::parse();
    config::save_config(&new_config, &args.config_path)
}

pub fn open_config_editor() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_default_config_path,
            get_version,
            get_config,
            save_config,
            stop,
            start,
            service_status,
            service_start,
            service_stop,
            service_restart,
            service_install,
            service_uninstall,
        ])
        .setup(|app| {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.set_title(&format!(
                    "Mouse Actions Config Editor v{}",
                    mouse_actions::process_args::get_version()
                ));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
