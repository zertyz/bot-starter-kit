use ogre_config_meld::OgreRootConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub repository: RepositoryConfig,
    pub dialog_processor: DialogProcessorConfig,
    pub telegram: TelegramConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryConfig {
    pub users_repository_db_file: String,
}
#[derive(derive_more::Debug, Clone, Serialize, Deserialize)]
pub struct DialogProcessorConfig {
    /// Maximum idle seconds before the dialog processor are closed
    /// -- controlling the effective "per user RAM session timeout".
    pub dialog_processor_idle_timeout: Duration,
    /// Minimum delay between MOs yielded to each user's dialog processor.
    pub per_user_mo_throttle_interval: Duration,
    /// The maximum time to wait for a clean shutdown
    pub shutdown_grace_period: Duration,
}

#[derive(derive_more::Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// To be automatically set to the `TELOXIDE_TOKEN` env var, if that is not present.
    #[debug("{}", "[REDACTED]")]
    pub teloxide_token: String,
    pub integration_mode: TelegramIntegrationMode,
}
#[derive(derive_more::Debug, Clone, Serialize, Deserialize)]
pub enum TelegramIntegrationMode {
    Polling,
    #[debug("{}", "[REDACTED]")]
    WebHook {
        url: String,
        secret: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: log::LevelFilter,
}

// impls
////////

impl BotConfig {
    pub const fn const_default() -> Self {
        Self {
            repository: RepositoryConfig { users_repository_db_file: String::new() },
            dialog_processor: DialogProcessorConfig {
                dialog_processor_idle_timeout: Duration::from_mins(30),
                per_user_mo_throttle_interval: Duration::from_secs(5),
                shutdown_grace_period: Duration::from_mins(1),
            },
            telegram: TelegramConfig {
                teloxide_token: String::new(),
                integration_mode: TelegramIntegrationMode::Polling,
            },
            logging: LoggingConfig { level: log::LevelFilter::Debug },
        }
    }
}
impl Default for BotConfig {
    fn default() -> Self {
        Self::const_default()
    }
}

impl OgreRootConfig for BotConfig {}
