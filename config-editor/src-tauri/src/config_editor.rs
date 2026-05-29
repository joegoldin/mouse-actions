use mouse_actions;
use mouse_actions::config;
use serde::Serialize;
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
    ServiceStatus {
        available: load == "loaded",
        active: active_state == "active",
        active_state,
        sub_state,
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
