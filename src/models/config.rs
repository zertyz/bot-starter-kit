use ogre_config_meld::OgreRootConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct BotConfig {
    pub repository_config: RepositoryConfig,
    pub telegram_config: TelegramConfig,
    pub logging_config: LoggingConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryConfig {
    pub users_repository_db_file: String,
}
#[derive(derive_more::Debug, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// To be automatically set to the `TELOXIDE_TOKEN` env var, if that is not present.
    #[debug("{}", "[REDACTED]")]
    pub teloxide_token: String,
    pub integration_mode: TelegramIntegrationMode,
    /// Maximum idle seconds before the dialog processor are closed
    /// -- controlling the effective "per user RAM session timeout".
    pub dialog_processor_idle_timeout: Duration,
}
#[derive(derive_more::Debug, Serialize, Deserialize, Clone)]
pub enum TelegramIntegrationMode {
    Polling,
    #[debug("{}", "[REDACTED]")]
    WebHook {
        url: String,
        secret: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: log::LevelFilter,
}

// impls
////////

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            repository_config: RepositoryConfig { users_repository_db_file: "./users_repository.redb".to_string() },
            telegram_config: TelegramConfig {
                teloxide_token: "".to_string(),
                integration_mode: TelegramIntegrationMode::Polling,
                dialog_processor_idle_timeout: Duration::from_mins(30),
            },
            logging_config: LoggingConfig { level: log::LevelFilter::Debug },
        }
    }
}

impl OgreRootConfig for BotConfig {}
