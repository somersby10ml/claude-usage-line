use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use regex::Regex;
use crate::cache::UsageData;
use crate::common;

fn parse_usage_output(clean_output: &str) -> Option<UsageData> {
    // Check for trust prompt
    if clean_output.contains("Do you trust the files in this folder?") {
        return None;
    }

    // Current session: "26% used"
    let session_usage_re = Regex::new(r"Current session[\s\S]*?(\d+)% used").unwrap();
    // Current session reset: "Resets 12:59am (Asia/Seoul)" -> "12:59am (Asia/Seoul)"
    let session_reset_re = Regex::new(r"Current session[\s\S]*?Resets ([^\n]+)").unwrap();

    // Current week: "28% used"
    let week_usage_re = Regex::new(r"Current week \(all models\)[\s\S]*?(\d+)% used").unwrap();
    // Current week reset: "Resets Oct 16, 1:59am (Asia/Seoul)" -> "Oct 16, 1:59am (Asia/Seoul)"
    let week_reset_re = Regex::new(r"Current week \(all models\)[\s\S]*?Resets ([^\n]+)").unwrap();

    let session_usage = session_usage_re.captures(clean_output)?.get(1)?.as_str();
    let session_reset = session_reset_re.captures(clean_output)?.get(1)?.as_str();
    let week_usage = week_usage_re.captures(clean_output)?.get(1)?.as_str();
    let week_reset = week_reset_re.captures(clean_output)?.get(1)?.as_str();

    Some(UsageData {
        current_session_used: format!("{}%", session_usage),
        current_session_reset_time: session_reset.to_string(),
        current_week_used: format!("{}%", week_usage),
        current_week_reset_time: week_reset.to_string(),
    })
}

/// Fetch and parse Claude usage data
pub fn fetch_and_parse_usage(custom_cli_path: Option<&str>) -> Result<UsageData, Box<dyn std::error::Error>> {
    common::log_debug("Starting usage capture")?;

    let work_dir = common::get_work_dir()?;
    let cli_path = common::get_claude_cli_path(custom_cli_path)?;

    common::log_debug(&format!("Using Claude CLI path: {}", cli_path))?;

    let pty_system = NativePtySystem::default();

    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut cmd = CommandBuilder::new("node");
    cmd.arg(&cli_path);
    cmd.arg("/usage");
    cmd.cwd(work_dir);
    // Prevent circular wake signal when status line hook triggers
    cmd.env("CLAUDE_USAGE_LINE_INTERNAL", "1");

    let mut child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);

    common::log_debug("PTY process spawned")?;

    let output = Arc::new(Mutex::new(String::new()));
    let output_clone = output.clone();

    let mut reader = pair.master.try_clone_reader()?;
    std::thread::spawn(move || {
        let mut buffer = [0u8; 1024];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buffer[..n]);
                    if let Ok(mut out) = output_clone.lock() {
                        out.push_str(&text);
                    }
                }
                Err(_) => break,
            }
        }
    });

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(15);
    let mut parsed_usage = None;

    loop {
        // Check if we can parse the output
        let current_output = output.lock().unwrap().clone();

        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(current_output.as_bytes());
        let screen = parser.screen();
        let screen_contents = screen.contents();

        let clean_bytes = strip_ansi_escapes::strip(&screen_contents);
        let clean_text = String::from_utf8_lossy(&clean_bytes);

        // Save parsed output to last_parse.txt in debug mode
        let _ = common::save_last_parse(&clean_text);

        if let Some(usage) = parse_usage_output(&clean_text) {
            common::log_debug("Successfully parsed usage data")?;
            parsed_usage = Some(usage);
            break;
        }

        // Check timeout
        if start.elapsed() > timeout {
            common::log_debug("Timeout reached without successful parse")?;
            break;
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    let _ = child.kill();

    parsed_usage.ok_or_else(|| "Failed to parse usage data".into())
}
