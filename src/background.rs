use fslock::LockFile;
use notify::{Watcher, RecursiveMode, RecommendedWatcher, EventKind};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use crate::common;
use crate::usage_capture;
use crate::cache;

/// Run the background process
pub fn run(refresh_min: u64, idle_min: u64, cli_path: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    common::log_debug("Background process started")?;

    let work_dir = common::get_work_dir()?;

    // Try to acquire lock
    let lock_path = work_dir.join("app.lock");
    let mut lock = LockFile::open(&lock_path)?;

    match lock.try_lock() {
        Ok(true) => {
            common::log_debug("Lock acquired - running as owner")?;
            run_as_owner(lock, refresh_min, idle_min, cli_path)?;
        }
        Ok(false) => {
            common::log_debug("Lock already held - sending wake signal")?;
            send_wake_signal()?;
        }
        Err(e) => {
            return Err(e.into());
        }
    }

    Ok(())
}

fn send_wake_signal() -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = common::get_work_dir()?;
    let signal_path = work_dir.join("wake.signal");

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    // Always write (creates if doesn't exist, overwrites if exists)
    std::fs::write(signal_path, timestamp.to_string())?;
    common::log_debug("Wake signal sent")?;

    Ok(())
}

fn run_as_owner(_lock: LockFile, refresh_min: u64, idle_min: u64, cli_path: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    common::log_debug("Starting owner mode")?;

    let work_dir = common::get_work_dir()?;
    let signal_path = work_dir.join("wake.signal");

    // Create wake.signal file if it doesn't exist
    if !signal_path.exists() {
        std::fs::write(&signal_path, "0")?;
        common::log_debug("Created wake.signal file")?;
    }

    // Setup file watcher
    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    watcher.watch(&signal_path, RecursiveMode::NonRecursive)?;

    common::log_debug("File watcher setup complete")?;

    let mut last_activity = Instant::now();
    let usage_interval = Duration::from_secs(refresh_min * 60);
    let idle_timeout = Duration::from_secs(idle_min * 60);
    let mut last_usage_check = Instant::now();

    // Initial usage collection
    common::log_debug("Performing initial usage collection")?;
    match usage_capture::fetch_and_parse_usage(cli_path.as_deref()) {
        Ok(usage_data) => {
            cache::save_to_cache(&usage_data)?;
        }
        Err(e) => {
            common::log_debug(&format!("Failed to fetch usage: {}", e))?;
        }
    }

    loop {
        // Check for wake signals
        match rx.try_recv() {
            Ok(Ok(event)) => {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        common::log_debug("Wake signal received - resetting idle timer")?;
                        last_activity = Instant::now();
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // Check if it's time to collect usage
        if last_usage_check.elapsed() >= usage_interval {
            common::log_debug("Collecting usage data (4min interval)")?;
            match usage_capture::fetch_and_parse_usage(cli_path.as_deref()) {
                Ok(usage_data) => {
                    cache::save_to_cache(&usage_data)?;
                }
                Err(e) => {
                    common::log_debug(&format!("Failed to fetch usage: {}", e))?;
                }
            }
            last_usage_check = Instant::now();
        }

        // Check idle timeout
        if last_activity.elapsed() >= idle_timeout {
            common::log_debug("Idle timeout reached - shutting down")?;
            break;
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    common::log_debug("Background process exiting")?;
    Ok(())
}
