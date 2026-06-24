use std::error::Error;
// This is a bot that asks you three questions, e.g. a simple test.
//
// # Example
// ```
//  - Hey
//  - Let's start! What's your full name?
//  - Gandalf the Grey
//  - How old are you?
//  - 223
//  - What's your location?
//  - Middle-earth
//  - Full name: Gandalf the Grey
//    Age: 223
//    Location: Middle-earth
// ```
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};
// use teloxide::macros::BotCommands;
use crate::models::config::BotConfig;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, Me};
use teloxide::utils::command::BotCommands;

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, Default)]
pub enum State {
    #[default]
    Start,
    ReceiveFullName,
    ReceiveAge {
        full_name: String,
    },
    ReceiveLocation {
        full_name: String,
        age: u8,
    },
}

#[derive(BotCommands)]
#[command(rename_rule = "lowercase")]
enum Command {
    /// Display this text
    Help,
    /// Start
    Start,
}

pub async fn run(config: &BotConfig) -> anyhow::Result<()> {
    println!("Starting dialogue bot...");

    unsafe {
        std::env::set_var(
            "TELOXIDE_TOKEN",
            config.telegram_config.teloxide_token.clone(),
        );
    }
    let bot = Bot::from_env();

    let handler = dptree::entry()
        // .branch(dptree::filter(|state: InMemStorage<State>| {
        //     println!("RECEIVED State: {state:?}");
        //     false
        // }))
        .branch(dptree::filter(|bot: Bot| {
            println!("RECEIVED bot: {bot:?}");
            false
        }))
        .branch(dptree::filter(|me: Me| {
            println!("RECEIVED me: {me:?}");
            false
        }))
        .branch(dptree::filter(|update: Update| {
            println!("RECEIVED update: {update:?}");
            false
        }))
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(stateless_commands),
        )
        .branch(
            Update::filter_message()
                .enter_dialogue::<Message, InMemStorage<State>, State>()
                .branch(dptree::case![State::Start].endpoint(start))
                .branch(dptree::case![State::ReceiveFullName].endpoint(receive_full_name))
                .branch(dptree::case![State::ReceiveAge { full_name }].endpoint(receive_age))
                .branch(
                    dptree::case![State::ReceiveLocation { full_name, age }]
                        .endpoint(receive_location),
                ),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![InMemStorage::<State>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

async fn stateless_commands(
    bot: Bot,
    mo_msg: Message,
    me: Me,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(text) = mo_msg.text() {
        match BotCommands::parse(text, me.username()) {
            Ok(Command::Help) => {
                bot.send_message(mo_msg.chat.id, Command::descriptions().to_string())
                    .await?;
            }
            Ok(Command::Start) => {
                bot.send_message(mo_msg.chat.id, "Settings:")
                    .reply_markup(show_settings())
                    .await?;
            }

            Err(err) => {
                let err_msg = format!(
                    "panic! -- stateless_commands() received an unrecognized command: {err}"
                );
                eprintln!("{err_msg}");
                bot.send_message(mo_msg.chat.id, err_msg).await?;
            }
        }
    }

    Ok(())
}

fn show_settings() -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    let debian_versions = [
        "Buzz", "Rex", "Bo", "Hamm", "Slink", "Potato", "Woody", "Sarge", "Etch", "Lenny",
        "Squeeze", "Wheezy", "Jessie", "Stretch", "Buster", "Bullseye",
    ];

    for versions in debian_versions.chunks(3) {
        let row = versions
            .iter()
            .map(|&version| InlineKeyboardButton::callback(version.to_owned(), version.to_owned()))
            .collect();

        keyboard.push(row);
    }

    InlineKeyboardMarkup::new(keyboard)
}

async fn start(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, "Let's start! What's your full name?")
        .await?;
    dialogue.update(State::ReceiveFullName).await?;
    Ok(())
}

async fn receive_full_name(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            bot.send_message(msg.chat.id, "How old are you?").await?;
            dialogue
                .update(State::ReceiveAge {
                    full_name: text.into(),
                })
                .await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Send me plain text.").await?;
        }
    }

    Ok(())
}

async fn receive_age(
    bot: Bot,
    dialogue: MyDialogue,
    full_name: String, // Available from `State::ReceiveAge`.
    msg: Message,
) -> HandlerResult {
    match msg.text().map(|text| text.parse::<u8>()) {
        Some(Ok(age)) => {
            bot.send_message(msg.chat.id, "What's your location?")
                .await?;
            dialogue
                .update(State::ReceiveLocation { full_name, age })
                .await?;
        }
        _ => {
            bot.send_message(msg.chat.id, "Send me a number.").await?;
        }
    }

    Ok(())
}

async fn receive_location(
    bot: Bot,
    dialogue: MyDialogue,
    (full_name, age): (String, u8), // Available from `State::ReceiveLocation`.
    msg: Message,
) -> HandlerResult {
    match msg.text() {
        Some(location) => {
            let report = format!("Full name: {full_name}\nAge: {age}\nLocation: {location}");
            bot.send_message(msg.chat.id, report).await?;
            dialogue.exit().await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Send me plain text.").await?;
        }
    }

    Ok(())
}
