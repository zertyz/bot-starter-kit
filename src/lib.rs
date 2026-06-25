#![doc = r#"
# Bot Starter Kit

The project diagram below is stored in `diagrams/bot-starter-kit-overview.svg`
and included in this crate-level documentation so it appears on the generated
rustdoc home page.
"#]
#![doc = r#"<div class="bot-starter-kit-diagram">"#]
#![doc = include_str!("../diagrams/bot-starter-kit-overview.svg")]
#![doc = r#"</div>"#]

pub mod commons;
pub mod db;
pub mod logic;
pub mod messaging;
pub mod models;
pub mod plot;
pub mod repository;
pub mod resources;
