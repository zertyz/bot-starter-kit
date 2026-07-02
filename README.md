# Rust's bot-starter-kit

**Work in progress:** this is an experimental backend Rust bot starter kit, not a production-ready framework yet. It is useful as a living template, reference implementation, and performance playground while the architecture is still being refined.

[![Main](https://github.com/zertyz/bot-starter-kit/actions/workflows/main.yml/badge.svg?branch=main)](https://github.com/zertyz/bot-starter-kit/actions/workflows/main.yml)
[![Docs](https://img.shields.io/badge/docs-GitHub%20Pages-blue)](https://zertyz.github.io/bot-starter-kit/doc/bot_starter_kit/)
[![Coverage report](https://img.shields.io/badge/coverage-report-blue)](https://zertyz.github.io/bot-starter-kit/coverage/html/)
[![Benchmark report](https://img.shields.io/badge/benchmarks-report-blue)](https://zertyz.github.io/bot-starter-kit/benchmarks/report/)

This repository collects patterns and boilerplate for building backend Rust bots that receive user messages, process them asynchronously, persist bot state, and send responses back through chat platforms. The current implementation is Telegram-first, using `teloxide`, while the internal contracts are shaped so more messaging platforms can be added later.

## At a glance

- Telegram bot starter using `teloxide`, Tokio, and stream-oriented message processing.
- Messaging contracts for separating mobile-originated inputs from mobile-terminated outputs.
- Async-facing database helpers for `heed`/LMDB, SQLite, and `redb`.
- Repository models and traits for user-oriented bot state.
- Encrypted configuration loading, command-line overrides, and `TELOXIDE_TOKEN` integration.
- Telegram demo scene with commands, inline buttons, progress updates, media replacement, benchmarks, and chart rendering.
- Criterion benchmark suites for channel handoff costs and database ingest/query comparisons.
- GitHub Pages publishing for rustdoc, coverage reports, and benchmark reports.
- Crate-level rustdoc support for inline SVG diagrams.

## Current feature snapshot

### Messaging and bot flow

- [X] Telegram gateway built on `teloxide`.
- [X] Per-user mobile-originated message streams for bot logic.
- [X] Mobile-terminated response stream consumption with configurable concurrency.
- [X] Telegram command handling and callback-query handling.
- [X] Helper functions for sending single Telegram requests or longer async response processes.
- [ ] Additional platform implementations such as WhatsApp, Slack, Teams, Discord, SMS, or Google Chat.

### Storage and repositories

- [X] Async-friendly `heed`/LMDB wrapper with read permit limiting and single-writer coordination.
- [X] Async-friendly `redb` wrapper with read permit limiting, write coordination, compaction support, and stream ingestion.
- [X] SQLite wrapper using `sqlx`, setup SQLs, stream ingestion, WAL maintenance, and maintenance hooks.
- [X] Common user and Telegram user repository traits and models.
- [X] Database benchmark paths shared between the Telegram demo and Criterion benches.
- [ ] Complete repository implementations for all declared user/session/config use cases.

### Configuration and runtime

- [X] Encrypted config-file loading and saving through `ogre-config-meld`.
- [X] Command-line parsing for showing, writing, and overriding the effective config.
- [X] `TELOXIDE_TOKEN` environment variable integration.
- [X] Redacted debug output for the Telegram bot token.
- [ ] Larger typed application config surface as the template grows.

### Demos, reporting, and performance work

- [X] Telegram demos for progress text edits, media replacement, inline menus, chat id display, and live chart simulation.
- [X] PNG chart rendering pipeline with layout, font metrics, move detection, and image encoding.
- [X] Criterion channel benchmarks for same-thread and inter-thread latency/throughput.
- [X] Criterion database benchmarks for SQLite, `redb`, and `heed`.
- [X] GitHub Actions checks for formatting, lints, tests, docs, coverage, benchmarks, and GitHub Pages publication.
- [X] Rustdoc home-page diagram support through SVG files stored in `diagrams/`.

## What this is not yet

- It is not a stable public framework API.
- It is not yet multi-platform, even though the messaging contracts are aiming in that direction.
- It does not yet provide complete production-ready repository coverage for every bot state pattern.
- It does not currently include the earlier planned Big-O analysis example as a surfaced feature.
- It should be reviewed, tested, and adapted before being used as the base for a production bot.
