# Rust's bot-starter-kit
**NOT YET** Production-ready template application for Rust Bots
to chat with Telegram, WhatsApp, Slack, Teams, Discord, SMS, ...

[![Rust](https://github.com/zertyz/bot-starter-kit/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/zertyz/bot-starter-kit/actions/workflows/rust.yml)
[![Docs](https://img.shields.io/badge/docs-GitHub%20Pages-blue)](https://zertyz.github.io/bot-starter-kit/doc/bot_starter_kit/)
[![Coverage report](https://img.shields.io/badge/coverage-report-blue)](https://zertyz.github.io/bot-starter-kit/coverage/html/)

This repository presents good patterns and all the needed boilerplate for creating backend Rust Bots suitable for high performance and attending to huge audiences.

Most likely, you'll need only a subset of the features provided by this template:
1. Features, Patterns, and Architectures:
- [X] Reactive model, where each user gets their own stream, for improved ergonomics
- [X] Wrappers for `heed`, `SQLite`, and `redb`
- [X] Included models and DB operations:
  - [X] Users
  - [X] Sessions -- for stateful applications
- ... complete this ... 
2. Powerful Configs and CLI integration:
- [X] Persistent config files using `ron` -- easily serializing any Rust type + automated DOCs generation
- [X] Support for Encryption of the config file -- minimizing the Risk of leaking BOT secrets
- [X] Command Line parsing & merging with the application configs
3. Testing:
- [X] Built-in example for Big-O analysis with `big-o-test` crate
- [X] Built-in example for Criterion bench
