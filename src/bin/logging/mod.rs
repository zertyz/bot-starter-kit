use anyhow::{Result, anyhow};
use ftlog::{LevelFilter, LoggerGuard};
use std::env;

const LOG_QUEUE_SIZE: usize = 8192;

pub fn init() -> Result<LoggerGuard> {
    ftlog::builder()
        .max_log_level(configured_level())
        .bounded(LOG_QUEUE_SIZE, false)
        .try_init()
        .map_err(|err| anyhow!("Could not initialize logging: {err}"))
}

fn configured_level() -> LevelFilter {
    env::var("BOT_STARTER_KIT_LOG")
        .ok()
        .or_else(|| env::var("RUST_LOG").ok())
        .as_deref()
        .and_then(parse_level_filter)
        .unwrap_or_else(default_level)
}

fn parse_level_filter(raw: &str) -> Option<LevelFilter> {
    raw.split(',')
        .find_map(|directive| {
            let level = directive
                .rsplit_once('=')
                .map(|(_, level)| level)
                .unwrap_or(directive)
                .trim()
                .to_ascii_lowercase();
            match level.as_str() {
                "off" => Some(LevelFilter::Off),
                "error" => Some(LevelFilter::Error),
                "warn" | "warning" => Some(LevelFilter::Warn),
                "info" => Some(LevelFilter::Info),
                "debug" => Some(LevelFilter::Debug),
                "trace" => Some(LevelFilter::Trace),
                _ => None,
            }
        })
}

fn default_level() -> LevelFilter {
    if cfg!(debug_assertions) { LevelFilter::Debug } else { LevelFilter::Info }
}
