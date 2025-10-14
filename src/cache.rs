use serde::{Serialize, Deserialize};
use std::fs;
use crate::common;

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheData {
    pub current_session_used: String,
    pub current_session_reset_time: String,
    pub current_week_used: String,
    pub current_week_reset_time: String,
    pub last_updated: String,
}

#[derive(Debug)]
pub struct UsageData {
    pub current_session_used: String,
    pub current_session_reset_time: String,
    pub current_week_used: String,
    pub current_week_reset_time: String,
}

/// Save usage data to cache.json
pub fn save_to_cache(data: &UsageData) -> Result<(), Box<dyn std::error::Error>> {
    let work_dir = common::get_work_dir()?;
    let cache_path = work_dir.join("cache.json");

    let cache = CacheData {
        current_session_used: data.current_session_used.clone(),
        current_session_reset_time: data.current_session_reset_time.clone(),
        current_week_used: data.current_week_used.clone(),
        current_week_reset_time: data.current_week_reset_time.clone(),
        last_updated: chrono::Utc::now().to_rfc3339(),
    };

    let json = serde_json::to_string_pretty(&cache)?;
    fs::write(cache_path, json)?;

    common::log_debug(&format!("Cache saved: Session {}, Week {}",
        data.current_session_used, data.current_week_used))?;

    Ok(())
}

/// Read cache and return formatted string (no newline)
/// Returns empty string if cache doesn't exist or is invalid
pub fn read_cache() -> String {
    let work_dir = match common::get_work_dir() {
        Ok(dir) => dir,
        Err(_) => return String::new(),
    };

    let cache_path = work_dir.join("cache.json");

    if !cache_path.exists() {
        return String::new();
    }

    let contents = match fs::read_to_string(cache_path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    let cache: CacheData = match serde_json::from_str(&contents) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    format_output(&cache)
}

/// Create a progress bar with filled and empty characters
fn create_progress_bar(percentage: u32, width: usize) -> String {
    let filled = (percentage as usize * width / 100).min(width);
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Format cache data with progress bars and colors
fn format_output(cache: &CacheData) -> String {
    // Always start with RESET
    let reset = "\x1b[0m";

    // Extract percentage values
    let current_percent: u32 = cache.current_session_used
        .trim_end_matches('%')
        .parse()
        .unwrap_or(0);
    let week_percent: u32 = cache.current_week_used
        .trim_end_matches('%')
        .parse()
        .unwrap_or(0);

    // Create progress bars (width: 20)
    let current_bar = create_progress_bar(current_percent, 20);
    let week_bar = create_progress_bar(week_percent, 20);

    // Dim color for subdued appearance
    let dim = "\x1b[2m";

    // Format output with fixed-width alignment (always colored, always small format)
    format!(
        "{}{}{:<8}: [{}] {:>3}% Resets {}\n{}{}{:<8}: [{}] {:>3}% Resets {}",
        reset, dim, "Current",
        current_bar, current_percent, cache.current_session_reset_time,
        reset, dim, "Week",
        week_bar, week_percent, cache.current_week_reset_time
    )
}
