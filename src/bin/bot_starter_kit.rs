mod logging;
mod models;

use anyhow::{Result, anyhow};
use bot_starter_kit::{logic::telegram_demoscene::run, models::config::*};
use ogre_config_meld::encryptable_tokio_fs::fs;
use ogre_config_meld::{CmdLineAndConfigIntegration, get_config_file_path, parse_cmdline_args};

use litcrypt::lc;
litcrypt::use_litcrypt!();

pub async fn parse_cmdline_and_merge_with_loaded_configs() -> Result<BotConfig> {
    fs::set_keys_from_passphrase(lc!("This secret string may only be revealed if one is debugging our code. An acceptable risk for our purposes.").as_ref());

    let cli_options: models::cli::CliOptions = parse_cmdline_args();
    let config_file_path = get_config_file_path::<models::cli::CliOptions, BotConfig>();
    let loaded_config_result = ogre_config_meld::load_from_file::<BotConfig>(&config_file_path).await;
    let config = match loaded_config_result {
        Ok(loaded_config) => {
            if let Some(loaded_config) = loaded_config {
                loaded_config
            } else {
                // config file not found -- we must have the token in the command line argument or env var.
                if let Some(_bot_token) = &cli_options.teloxide_token {
                    if !cli_options.write_effective_config {
                        eprintln!("Couldn't find the configuration file {config_file_path:?}.");
                        eprintln!(
                            "In addition to passing in the -t option or the TELOXIDE_TOKEN env var, also specify -w to create an encrypted config file allowing you to drop the token and no longer using these options."
                        );
                        Err(anyhow!("No Teloxide token found"))?;
                    }
                } else {
                    eprintln!("Couldn't find the configuration file {config_file_path:?}.");
                    eprintln!("Please, re-run and specify the -t option or the TELOXIDE_TOKEN env var; also specify -w to create an encrypted config file allowing you to drop the token.");
                    Err(anyhow!("No Teloxide token found"))?;
                }
                BotConfig::default()
            }
        }
        Err(err) => Err(anyhow!("Error loading the encrypted config file: {err}"))?,
    };

    let show_effective_config = cli_options.show_effective_config;
    let write_effective_config = cli_options.write_effective_config;
    let config = cli_options.merge_with_config(config)?;
    if show_effective_config {
        eprintln!("EFFECTIVE CONFIG: {:#?}", config);
    }
    if write_effective_config {
        ogre_config_meld::save_to_file(&config, "", &config_file_path)
            .await
            .map_err(|err| anyhow!("Couldn't save the config file: {err}"))?;
        eprintln!("Configuration file saved successfully to {config_file_path:?}. Exiting. Re-run the program without -w, -t, and without the `TELOXIDE_TOKEN` env var.");
        std::process::exit(0);
    }
    Ok(config)
}
#[tokio::main]
async fn main() -> Result<()> {
    let config = parse_cmdline_and_merge_with_loaded_configs().await?;
    let _logging_guard = logging::init(&config.logging)?;
    run(config).await
}
