use super::{
    config::{PngEncodeMode, Theme},
    data::{fetch_bcb_usd_brl_quotes, synthetic_quotes},
    moves::detect_important_moves,
    render::RenderEngine,
    timing::Timings,
};
use crate::plot::data::Quote;
use anyhow::{Result, anyhow};
use std::env;
use std::time::Duration;
use teloxide::Bot;
use teloxide::payloads::SendPhotoSetters;
use teloxide::prelude::{ChatId, Message, Requester};
use teloxide::types::{InputFile, InputMedia, InputMediaPhoto};

#[derive(Clone, Debug)]
struct Cli {
    output: String,
    synthetic: bool,
    threshold_cents: Option<i32>,
    png_mode: Option<PngEncodeMode>,
}

async fn plot(bot: &Bot, chat_id: ChatId, engine: &mut RenderEngine, message: &mut Option<Message>, quotes: &[Quote]) -> Result<()> {
    let generated_png = "/tmp/brl2usd.png";
    let mut timings = Timings::default();
    let theme = Theme::default();

    let moves = timings.measure("detect moves", || {
        Ok(detect_important_moves(
            quotes,
            theme
                .movement
                .important_delta_cents,
        ))
    })?;

    let plan = timings.measure("layout", || engine.plan(quotes, &moves))?;
    let rgb = timings.measure("rasterize rgb", || engine.rasterize_rgb(&plan))?;
    let (png, png_stats) = timings.measure("encode png", || engine.encode_png(rgb))?;
    timings.measure("write png", || engine.write_png(generated_png, &png))?;

    let caption = format!(
        "generated {} with {} quotes, {} important move(s), metric-cache={} entries, png-mode={}, png={} bytes",
        generated_png,
        quotes.len(),
        moves.len(),
        engine.metric_cache_len(),
        png_stats.mode,
        png_stats.png_bytes,
    );

    match message.as_ref() {
        None => {
            // first message -- send it
            let m = bot
                .send_photo(chat_id, InputFile::file(generated_png))
                .caption(format!("FIRST! {caption}"))
                .await?;
            message.replace(m);
        }
        Some(m) => {
            // continuation -- edit the first message
            let media = InputMedia::Photo(InputMediaPhoto::new(InputFile::file(generated_png)).caption(caption.to_string()));
            bot.edit_message_media(chat_id, m.id, media)
                .await?;
        }
    }

    Ok(())
}

pub async fn demo(bot: &Bot, chat_id: ChatId) -> Result<()> {
    let cli = parse_cli()?;
    let mut timings = Timings::default();
    let mut message = None;

    let mut theme = Theme::default();
    if let Some(threshold) = cli.threshold_cents {
        theme
            .movement
            .important_delta_cents = threshold;
    }
    if let Some(mode) = cli.png_mode {
        theme
            .png
            .mode = mode;
    }

    let quotes = timings
        .measure_async("load data", || async {
            if cli.synthetic {
                Ok(synthetic_quotes())
            } else {
                match fetch_bcb_usd_brl_quotes(90).await {
                    Ok(quotes) => Ok(quotes),
                    Err(err) => {
                        log::warn!("BCB API failed; using synthetic data instead: {err}");
                        Ok(synthetic_quotes())
                    }
                }
            }
        })
        .await?;

    let moves = timings.measure("detect moves", || {
        Ok(detect_important_moves(
            &quotes,
            theme
                .movement
                .important_delta_cents,
        ))
    })?;
    let mut engine = RenderEngine::new(theme);
    timings.measure("warm up", || {
        engine.warm_up(&quotes, &moves);
        Ok(())
    })?;
    for len in 3..=quotes.len() {
        plot(bot, chat_id, &mut engine, &mut message, &quotes[0..len]).await?;
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    //bot.edit_message_caption(chat_id, m.id).caption("✅ Render complete.").await?;
    Ok(())
}

fn parse_cli() -> Result<Cli> {
    let mut cli = Cli {
        output: "/tmp/usd_brl.png".to_string(),
        synthetic: false,
        threshold_cents: None,
        png_mode: Some(PngEncodeMode::Balanced),
    };

    for arg in env::args().skip(1) {
        if arg == "--synthetic" {
            cli.synthetic = true;
        } else if arg == "--no-optimize" {
            // Backward-compatible alias from v5. The v6 default is already fast single-pass.
            cli.png_mode = Some(PngEncodeMode::Fast);
        } else if let Some(v) = arg.strip_prefix("--output=") {
            cli.output = v.to_string();
        } else if let Some(v) = arg.strip_prefix("--threshold-cents=") {
            let cents: i32 = v.parse()?;
            if cents <= 0 {
                return Err(anyhow!("--threshold-cents must be positive"));
            }
            cli.threshold_cents = Some(cents);
        } else if let Some(v) = arg.strip_prefix("--optimize-preset=") {
            // Backward-compatible alias from v5, now using oxipng directly from raw pixels.
            let preset: u8 = v.parse()?;
            if preset > 6 {
                return Err(anyhow!("--optimize-preset must be between 0 and 6"));
            }
            cli.png_mode = Some(PngEncodeMode::OxipngRaw { preset });
        } else if let Some(v) = arg.strip_prefix("--png-mode=") {
            cli.png_mode = Some(parse_png_mode(v)?);
        } else {
            return Err(anyhow!("unknown argument: {arg}"));
        }
    }

    Ok(cli)
}

fn parse_png_mode(value: &str) -> Result<PngEncodeMode> {
    match value {
        "fast" => Ok(PngEncodeMode::Fast),
        "balanced" | "default" => Ok(PngEncodeMode::Balanced),
        "uncompressed" => Ok(PngEncodeMode::Uncompressed),
        _ => {
            if let Some(v) = value.strip_prefix("level:") {
                let level: u8 = v.parse()?;
                if !(1..=9).contains(&level) {
                    return Err(anyhow!("--png-mode=level:N requires N between 1 and 9"));
                }
                Ok(PngEncodeMode::Level(level))
            } else if let Some(v) = value.strip_prefix("oxipng-raw:") {
                let preset: u8 = v.parse()?;
                if preset > 6 {
                    return Err(anyhow!("--png-mode=oxipng-raw:N requires N between 0 and 6"));
                }
                Ok(PngEncodeMode::OxipngRaw { preset })
            } else {
                Err(anyhow!("--png-mode must be fast, balanced, uncompressed, level:N, or oxipng-raw:N"))
            }
        }
    }
}
