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

    /// Public PEM certificate file used by the Telegram webhook HTTPS server.
    #[clap(long, env = "TELOXIDE_WEBHOOK_CERTIFICATE_FILE")]
    pub telegram_webhook_certificate_file: Option<String>,

    /// Private key PEM file used by the Telegram webhook HTTPS server.
    #[clap(long, env = "TELOXIDE_WEBHOOK_PRIVATE_KEY_FILE")]
    pub telegram_webhook_private_key_file: Option<String>,

    /// Maximum idle seconds before the per-user dialog processor (a.k.a., session) is closed.
    #[clap(long)]
    pub dialog_idle_timeout_secs: Option<u64>,

    /// Minimum seconds between MOs yielded to each user's dialog processor.
    #[clap(long)]
    pub per_user_mo_throttle_interval_secs: Option<u64>,

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

        self.per_user_mo_throttle_interval_secs
            .inspect(|per_user_mo_throttle_interval_secs| {
                config
                    .dialog_processor
                    .per_user_mo_throttle_interval = Duration::from_secs(*per_user_mo_throttle_interval_secs)
            });

        self.shutdown_grace_period_secs
            .inspect(|shutdown_grace_period_secs| {
                config
                    .dialog_processor
                    .shutdown_grace_period = Duration::from_secs(*shutdown_grace_period_secs)
            });

        // Telegram in WebHook mode
        let telegram_webhook_url = self
            .telegram_webhook_url
            .filter(|telegram_webhook_url| {
                !telegram_webhook_url
                    .trim()
                    .is_empty()
            });
        let telegram_webhook_secret = self
            .telegram_webhook_secret
            .filter(|telegram_webhook_secret| {
                !telegram_webhook_secret
                    .trim()
                    .is_empty()
            });
        let telegram_certificate_file = self
            .telegram_webhook_certificate_file
            .filter(|telegram_certificate_file| {
                !telegram_certificate_file
                    .trim()
                    .is_empty()
            });
        let telegram_certificate_key_file = self
            .telegram_webhook_private_key_file
            .filter(|telegram_certificate_key_file| {
                !telegram_certificate_key_file
                    .trim()
                    .is_empty()
            });
        match (telegram_webhook_url, telegram_webhook_secret, telegram_certificate_file, telegram_certificate_key_file) {
            (Some(telegram_webhook_url), Some(telegram_webhook_secret), Some(telegram_certificate_file), Some(telegram_certificate_key_file)) => {
                config
                .telegram
                .integration_mode = TelegramIntegrationMode::WebHook {
                    url: telegram_webhook_url.to_string(),
                    secret: telegram_webhook_secret.to_string(),
                    certificate_file: telegram_certificate_file.to_string(),
                    private_key_file: telegram_certificate_key_file.to_string(),
                };
            }
            (None, None, None, None) => {
                /*config
                    .telegram
                    .integration_mode = TelegramIntegrationMode::Polling;*/
            }
            _ => {
                Err(ogre_config_meld::Error::MergingLogicViolation { message: "Configuration error: when specifying one of these options, all of them should be specified: `--telegram_webhook_url`, `--telegram_webhook_secret`, --telegram_webhook_certificate_file, --telegram_webhook_private_key_file".to_string() })?
            }
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ogre_config_meld::CmdLineAndConfigIntegration;
    use bot_starter_kit::models::config::TelegramConfig;

    #[test]
    fn merge_dialog_processor() {
        let expected_dialog_processor_idle_timeout = Duration::from_secs(7);
        let expected_per_user_mo_throttle_interval = Duration::from_secs(6);
        let expected_shutdown_grace_period = Duration::from_secs(8);
        let config = CliOptions {
            write_effective_config: false,
            show_effective_config: false,
            log_level: None,
            teloxide_token: None,
            telegram_webhook_url: None,
            telegram_webhook_secret: None,
            telegram_webhook_certificate_file: None,
            telegram_webhook_private_key_file: None,
            dialog_idle_timeout_secs: Some(expected_dialog_processor_idle_timeout.as_secs()),
            per_user_mo_throttle_interval_secs: Some(expected_per_user_mo_throttle_interval.as_secs()),
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
                .per_user_mo_throttle_interval,
            expected_per_user_mo_throttle_interval,
            "Merging logic failed for `per_user_mo_throttle_interval`"
        );
        assert_eq!(
            config
                .dialog_processor
                .shutdown_grace_period,
            expected_shutdown_grace_period,
            "Merging logic failed for `shutdown_grace_period`"
        );
    }

    #[test]
    fn merge_telegram_webhook_options() {
        let expected_telegram_webhook_url = "https://1.2.3.4:8443/";
        let expected_telegram_webhook_secret = "my own secret";
        let expected_certificate_file = "certs/public.pem";
        let expected_private_key_file = "certs/private.key";
        let config = CliOptions {
            write_effective_config: false,
            show_effective_config: false,
            log_level: None,
            teloxide_token: None,
            telegram_webhook_url: Some(expected_telegram_webhook_url.to_string()),
            telegram_webhook_secret: Some(expected_telegram_webhook_secret.to_string()),
            telegram_webhook_certificate_file: Some(expected_certificate_file.to_string()),
            telegram_webhook_private_key_file: Some(expected_private_key_file.to_string()),
            dialog_idle_timeout_secs: None,
            per_user_mo_throttle_interval_secs: None,
            shutdown_grace_period_secs: None,
        }
        .merge_with_config(BotConfig::default())
        .expect("CLI merge should succeed");

        match config
            .telegram
            .integration_mode
        {
            TelegramIntegrationMode::Polling => panic!("Config is still in the POLLING mode. Merging likely missed a lot of values"),
            TelegramIntegrationMode::WebHook { url, secret, certificate_file, private_key_file } => {
                assert_eq!(url, expected_telegram_webhook_url, "Merging logic failed for `telegram_webhook_url`");
                assert_eq!(secret, expected_telegram_webhook_secret, "Merging logic failed for `telegram_webhook_secret`");
                assert_eq!(certificate_file, expected_certificate_file, "Merging logic failed for `telegram_webhook_certificate_file`");
                assert_eq!(&private_key_file, expected_private_key_file, "Merging logic failed for `telegram_webhook_private_key_file`");
            }
        }
    }

    #[test]
    fn merging_preserves_webhook_mode() {
        let config_from_file = BotConfig {
            telegram: TelegramConfig {
                teloxide_token: "my-secret-token".to_string(),
                integration_mode: TelegramIntegrationMode::WebHook {
                    url: "my-url".to_string(),
                    secret: "really-a-secret".to_string(),
                    certificate_file: "/a/b/c".to_string(),
                    private_key_file: "/a/b/c/d".to_string(),
                }
            },
            ..BotConfig::default()
        };
        let cli_options = CliOptions {
            write_effective_config: false,
            show_effective_config: false,
            log_level: None,
            teloxide_token: None,
            telegram_webhook_url: None,
            telegram_webhook_secret: None,
            telegram_webhook_certificate_file: None,
            telegram_webhook_private_key_file: None,
            dialog_idle_timeout_secs: None,
            per_user_mo_throttle_interval_secs: None,
            shutdown_grace_period_secs: None,
        };
        let effective_config = cli_options.merge_with_config(config_from_file)
            .expect("CLI merge should succeed");

        assert!(matches!(effective_config.telegram.integration_mode, TelegramIntegrationMode::WebHook {..}), "Polling mode, somehow, sneaked in again!");
    }
}
