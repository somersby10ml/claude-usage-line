use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::process::{Command, Stdio};
use chrono::Local;

static DEBUG_MODE: AtomicBool = AtomicBool::new(false);

/// Get the working directory for all app files
/// Windows: %LOCALAPPDATA%\claude-usage-line
/// Linux/Mac: ~/.cache/claude-usage-line
pub fn get_work_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let work_dir = if cfg!(windows) {
        // Windows: %LOCALAPPDATA%\claude-usage-line
        let local_app_data = env::var("LOCALAPPDATA")
            .map_err(|_| "LOCALAPPDATA environment variable not found")?;
        PathBuf::from(local_app_data).join("claude-usage-line")
    } else {
        // Linux/Mac: ~/.cache/claude-usage-line
        let home = env::var("HOME")
            .map_err(|_| "HOME environment variable not found")?;
        PathBuf::from(home).join(".cache").join("claude-usage-line")
    };

    if !work_dir.exists() {
        fs::create_dir_all(&work_dir)?;
    }

    Ok(work_dir)
}

/// Enable debug logging
pub fn enable_debug_logging() -> Result<(), Box<dyn std::error::Error>> {
    DEBUG_MODE.store(true, Ordering::Relaxed);
    log_debug("Debug logging enabled")?;
    Ok(())
}

/// Log a debug message (appends to debug.log)
pub fn log_debug(message: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !DEBUG_MODE.load(Ordering::Relaxed) {
        return Ok(());
    }

    let work_dir = get_work_dir()?;
    let log_path = work_dir.join("debug.log");

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_line = format!("[{}] {}\n", timestamp, message);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    file.write_all(log_line.as_bytes())?;
    Ok(())
}

/// Save parsed output to last_parse.txt (overwrites, only in debug mode)
pub fn save_last_parse(content: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !DEBUG_MODE.load(Ordering::Relaxed) {
        return Ok(());
    }

    let work_dir = get_work_dir()?;
    let parse_path = work_dir.join("last_parse.txt");

    fs::write(&parse_path, content)?;
    Ok(())
}

/// Get Claude CLI path by querying npm global root
/// If custom_path is provided, use it directly; otherwise query npm
pub fn get_claude_cli_path(custom_path: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
    // If custom path is provided, use it directly
    if let Some(path) = custom_path {
        return Ok(path.to_string());
    }

    // Otherwise, query npm global root
    let npm_prog = if cfg!(windows) { "npm.cmd" } else { "npm" };

    let output = Command::new(npm_prog)
        .args(["root", "-g"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("npm root -g failed: {}", stderr).into());
    }

    let npm_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let cli_path = format!("{}/@anthropic-ai/claude-code/cli.js", npm_root);

    Ok(cli_path)
}
