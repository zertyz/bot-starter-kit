use bot_starter_kit::models::config::{BotConfig, TelegramConfig};
use ogre_config_meld::clap;

/// Command Line Options
#[derive(clap::Parser, Debug)]
pub struct CliOptions {
    /// (Re)Writes the configuration file.
    ///
    /// If it doesn't exist, one will be created with the default settings -- in which case, the `-t` option is required;
    ///
    /// If it already exists, any given cmd-line options will be merged into the current config contents and the new data
    /// re-encrypted and rewritten.
    ///
    /// After writing the config, the program will exit without doing any other operation.
    #[clap(long, short = 'w')]
    pub write_effective_config: bool,

    /// Dumps the configuration in place -- prints the merged result from the config file + command-line options.
    #[clap(long, short = 's')]
    pub show_effective_config: bool,

    /// Specifies the Telegram's bot token to be put used as `TELOXIDE_TOKEN`.
    ///
    /// When this option is used with the `-w` option, the program's configuration file will be updated
    /// (or created) and then encrypted. On subsequent runs, you don't need to provide this option neither keep the token around.
    ///
    /// Security suggestion: prefer specifying the env var `TELOXIDE_TOKEN` instead of passing it as a command-line option.
    /// Both are not great, but the command-line option appears, by default, in more places -- such as the shell history, ps dumps, ...
    #[clap(long, short = 't', env = "TELOXIDE_TOKEN")]
    pub teloxide_token: Option<String>,
}

impl ogre_config_meld::CmdLineAndConfigIntegration<BotConfig> for CliOptions {
    fn config_file_path(&self) -> Option<&str> {
        None
    }

    fn should_write_effective_config(&self) -> bool {
        self.write_effective_config
    }

    fn should_show_effective_config(&self) -> bool {
        self.show_effective_config
    }

    fn merge_with_config(self, config: BotConfig) -> BotConfig {
        if let Some(teloxide_token) = &self.teloxide_token {
            BotConfig {
                repository_config: config.repository_config,
                telegram_config: TelegramConfig {
                    teloxide_token: teloxide_token.to_string(),
                },
            }
        } else {
            config
        }
    }
}
