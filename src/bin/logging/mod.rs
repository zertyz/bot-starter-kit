use anyhow::{Result, anyhow};
use bot_starter_kit::models::config::LoggingConfig;
use ftlog::LoggerGuard;

const LOG_QUEUE_SIZE: usize = 8192;

pub fn init(config: &LoggingConfig) -> Result<LoggerGuard> {
    ftlog::builder()
        .max_log_level(config.level)
        .bounded(LOG_QUEUE_SIZE, false)
        .try_init()
        .map_err(|err| anyhow!("Could not initialize logging: {err}"))
}
