//! Contains code used across different tests

pub mod db_test_commons;

// enable logging for unit tests
#[cfg(test)]
#[ctor::ctor(unsafe)]
static TEST_LOGGER_GUARD: ftlog::LoggerGuard = {
    ftlog::builder()
        .max_log_level(log::LevelFilter::Info)
        .bounded(16, true)
        .try_init()
        .expect("could not initialize logging for tests")
};
