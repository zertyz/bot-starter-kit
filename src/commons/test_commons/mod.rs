//! Contains code used across different tests

use anyhow::{anyhow, Result};

pub mod db_test_commons;

// enable logging for unit tests
#[cfg(test)]
#[ctor::ctor(unsafe)]
static TEST_LOGGER_GUARD: Result<ftlog::LoggerGuard> = {
    ftlog::builder()
        .max_log_level(log::LevelFilter::Info)
        .bounded(16, true)
        .try_init()
        .map_err(|err| anyhow!("Could not initialize logging for tests: {err}"))
        .inspect_err(|err| eprintln!("### {err}"))
};
