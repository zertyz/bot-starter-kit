use ogre_config_meld::OgreRootConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct BotConfig {
    pub repository_config: RepositoryConfig,
    pub telegram_config: TelegramConfig,
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
}

// impls
////////

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            repository_config: RepositoryConfig {
                users_repository_db_file: "./users_repository.redb".to_string(),
            },
            telegram_config: TelegramConfig {
                teloxide_token: "".to_string(),
            },
        }
    }
}

impl OgreRootConfig for BotConfig {}
