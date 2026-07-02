use bot_starter_kit::models::config::{BotConfig, TelegramIntegrationMode};
use ogre_config_meld::clap;
use std::time::Duration;

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

    /// Specifies the maximum verbosity for this program's logs.
    #[clap(long, env = "LOG_LEVEL")]
    pub log_level: Option<log::LevelFilter>,

    /// Specifies the Telegram's bot token to be used as `TELOXIDE_TOKEN`.
    ///
    /// When this option is used with the `-w` option, the program's configuration file will be updated
    /// (or created) and then encrypted. On later runs, you don't need to provide this option neither keep the token around.
    ///
    /// Security suggestion: prefer specifying the env var `TELOXIDE_TOKEN` instead of passing it as a command-line option.
    /// Both are not great, but the command-line option appears, by default, in more places -- such as the shell history, ps dumps, ...
    #[clap(long, short = 't', env = "TELOXIDE_TOKEN")]
    pub teloxide_token: Option<String>,

    /// If present, specifies that `teloxide` should communicate with the Telegram servers through the "WebHook" mode,
    /// and that the given value represents the https address that reaches this node & program.
    /// See also `--telegram_webhook_secret`, which should be specified for extra-security.
    ///
    /// If absent, "Polling" mode will be used -- ideal for testing & staging: more portable, but less reliable.
    #[clap(long, env = "TELOXIDE_WEBHOOK_URL")]
    pub telegram_webhook_url: Option<String>,

    /// If present, specifies that `teloxide` should communicate with the Telegram servers through the "WebHook" mode,
    /// and that the given value specifies the token to be added to each call header, for extra security.
    /// You also need to specify `--telegram_webhook_url`.
    ///
    /// If absent, "Polling" mode will be used -- ideal for testing & staging: more portable, but less reliable.
    #[clap(long, env = "TELOXIDE_WEBHOOK_SECRET")]
    pub telegram_webhook_secret: Option<String>,

    /// Maximum idle seconds before the per-user dialog processor (a.k.a., session) is closed.
    #[clap(long)]
    pub dialog_idle_timeout_secs: Option<u64>,

    /// The maximum time to wait for a clean shutdown -- in seconds
    #[clap(long)]
    pub shutdown_grace_period_secs: Option<u64>,
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

    fn merge_with_config(self, config: BotConfig) -> Result<BotConfig, ogre_config_meld::Error> {
        let mut config = config;

        self.log_level
            .inspect(|log_level| {
                config
                    .logging
                    .level = *log_level
            });

        self.teloxide_token
            .inspect(|teloxide_token| {
                config
                    .telegram
                    .teloxide_token = teloxide_token.clone()
            });

        self.dialog_idle_timeout_secs
            .inspect(|dialog_idle_timeout_secs| {
                config
                    .dialog_processor
                    .dialog_processor_idle_timeout = Duration::from_secs(*dialog_idle_timeout_secs)
            });

        if self
            .telegram_webhook_url
            .is_some()
            || self
                .telegram_webhook_secret
                .is_some()
        {
            let Some(telegram_webhook_url) = self
                .telegram_webhook_url
                .as_ref()
            else {
                return Err(ogre_config_meld::Error::MergingLogicViolation { message: "CLI parameter `--telegram_webhook_url` is missing".to_string() });
            };
            let Some(telegram_webhook_secret) = self
                .telegram_webhook_secret
                .as_ref()
            else {
                return Err(ogre_config_meld::Error::MergingLogicViolation { message: "CLI parameter `--telegram_webhook_secret` is missing".to_string() });
            };
            config
                .telegram
                .integration_mode = TelegramIntegrationMode::WebHook {
                url: telegram_webhook_url.to_string(),
                secret: telegram_webhook_secret.to_string(),
            };
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ogre_config_meld::CmdLineAndConfigIntegration;

    #[test]
    fn merge_dialog_idle_timeout_secs() {
        let expected_dialog_processor_idle_timeout = Duration::from_secs(7);
        let expected_shutdown_grace_period = Duration::from_secs(8);
        let config = CliOptions {
            write_effective_config: false,
            show_effective_config: false,
            log_level: None,
            teloxide_token: None,
            telegram_webhook_url: None,
            telegram_webhook_secret: None,
            dialog_idle_timeout_secs: Some(expected_dialog_processor_idle_timeout.as_secs()),
            shutdown_grace_period_secs: Some(expected_shutdown_grace_period.as_secs()),
        }
        .merge_with_config(BotConfig::default())
        .expect("CLI merge should succeed");

        assert_eq!(
            config
                .dialog_processor
                .dialog_processor_idle_timeout,
            expected_dialog_processor_idle_timeout,
            "Merging logic failed for `dialog_processor_idle_timeout`"
        );
        assert_eq!(
            config
                .dialog_processor
                .shutdown_grace_period,
            expected_shutdown_grace_period,
            "Merging logic failed for `shutdown_grace_period`"
        );
    }
}
