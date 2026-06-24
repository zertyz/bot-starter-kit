# **NOT YET** Production-ready seed & template application for Rust Bots to chat with Telegram, WhatsApp, Slack, Teams, Discord, SMS, ...

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
