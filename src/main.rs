use std::env;
use std::process::{Command, Stdio};
use clap::Parser;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod usage_capture;
mod background;
mod cache;
mod common;

#[derive(Parser, Debug)]
#[command(name = "ccusage-api")]
#[command(about = "Claude CLI usage statistics daemon", long_about = None)]
struct Args {
    /// Run as background daemon
    #[arg(long)]
    daemon: bool,

    /// Enable debug logging
    #[arg(long)]
    debug: bool,

    /// Usage refresh interval in minutes
    #[arg(long, default_value = "4")]
    refresh_interval: u64,

    /// Idle timeout in minutes before daemon shuts down
    #[arg(long, default_value = "10")]
    idle_timeout: u64,

    /// Custom path to Claude Code cli.js
    #[arg(long)]
    ccjs: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.daemon {
        // Skip if called from internal usage capture (prevents circular wake signal)
        if env::var("CLAUDE_USAGE_LINE_INTERNAL").is_ok() {
            return Ok(());
        }

        // Daemon mode: enable debug logging if requested, then run background process
        if args.debug {
            common::enable_debug_logging()?;
        }
        background::run(args.refresh_interval, args.idle_timeout, args.ccjs)?;
    } else {
        // User mode: spawn daemon process, then read and print cache
        spawn_daemon(args.debug, args.refresh_interval, args.idle_timeout, args.ccjs.as_deref())?;

        // Small delay to let daemon start
        std::thread::sleep(std::time::Duration::from_millis(100));

        let output = cache::read_cache();
        print!("{}", output);
    }

    Ok(())
}

/// Test function: Execute 'claude /usage' via PTY and print raw output
fn test() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Starting PTY test with 'node cli.js /usage' ===\n");
    println!("Finding npm global directory...");

    // Windows에서는 npm.cmd를 우선 시도
    let npm_prog = if cfg!(windows) { "npm.cmd" } else { "npm" };

    let output_res = Command::new(npm_prog)
        .args(["root", "-g"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // 디버깅을 위해 stderr 확보
        .output();

    let npm_root = match output_res {
        Ok(out) => {
            if !out.status.success() {
                eprintln!("'npm root -g' exited with: {}", out.status);
                eprintln!("stderr: {}", String::from_utf8_lossy(&out.stderr));
                return Err("npm returned non-zero exit status".into());
            }
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        Err(e) => {
            eprintln!("Failed to execute '{}': {}", npm_prog, e);
            return Err(format!("Failed to execute '{}': {}", npm_prog, e).into());
        }
    };

    println!("npm global root: {}", npm_root);

    // Step 2: Build path to cli.js
    let cli_path = format!("{}/@anthropic-ai/claude-code/cli.js", npm_root);
    println!("cli.js path: {}\n", cli_path);

    // Step 3: Execute via PTY
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

    println!("Spawning command: node {} /usage", cli_path);

    let mut child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);

    println!("PTY process spawned successfully\n");

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

    println!("Waiting for output (15s timeout)...\n");

    loop {
        let current_output = output.lock().unwrap().clone();

        if !current_output.is_empty() {
            println!("--- Raw PTY Output (with ANSI codes) ---");
            println!("{}", current_output);
            println!("--- End of Raw Output ---\n");

            // Parse with vt100 and strip ANSI
            let mut parser = vt100::Parser::new(24, 80, 0);
            parser.process(current_output.as_bytes());
            let screen = parser.screen();
            let screen_contents = screen.contents();

            let clean_bytes = strip_ansi_escapes::strip(&screen_contents);
            let clean_text = String::from_utf8_lossy(&clean_bytes);

            println!("--- Cleaned Output (ANSI stripped) ---");
            println!("{}", clean_text);
            println!("--- End of Cleaned Output ---");

            break;
        }

        if start.elapsed() > timeout {
            println!("Timeout reached without output");
            break;
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    let _ = child.kill();
    println!("\n=== Test completed ===");

    Ok(())
}

/// Spawn a detached daemon process
fn spawn_daemon(debug_mode: bool, refresh_min: u64, idle_min: u64, ccjs: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let current_exe = env::current_exe()?;

    let mut cmd = Command::new(current_exe);
    cmd.arg("--daemon");

    // Set cwd to work dir so daemon doesn't hold project folder
    if let Ok(work_dir) = common::get_work_dir() {
        cmd.current_dir(work_dir);
    }

    if debug_mode {
        cmd.arg("--debug");
    }

    // Explicitly pass refresh-interval and idle-timeout values
    cmd.arg("--refresh-interval").arg(refresh_min.to_string());
    cmd.arg("--idle-timeout").arg(idle_min.to_string());

    // Pass custom cli.js path if provided
    if let Some(path) = ccjs {
        cmd.arg("--ccjs").arg(path);
    }

    // Platform-specific detached process creation
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        cmd.creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)  // Create new process group
            .spawn()?;
    }

    Ok(())
}
