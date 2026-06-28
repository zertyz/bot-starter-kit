# Code Patterns

This document contains HOW-TO instructions when implementing/extending each of the topics below.
This document can be viewed as a lighter version of `CODE_GUIDELINES.md` -- here you'll see guidelines with no enforcement.

## 1. Configuration
 In the production artifacts, all configuration sources -- be them:
* Environment variables,
* CLI options,
* Data from external files;
should be unified through the crate `ogre-config-meld`, using the models in `src/models/config.rs` and merged by
`parse_cmdline_and_merge_with_loaded_configs()` in the main module.
1. Should an external crate require an ENV var, we should use `env::set_var()` to allow defining it from the config models 

## 2. Database Wrappers
All database wrappers in `src/db` should:
1. Allow the underlying database to work in async without stalling the async workers -- e.g., waiting on a sync mutex to enforce a single writer. Being briefly blocked by low-latency database IO is acceptable.
2. Provide means to receive query results as a Stream, if applicable.
3. Provide means to ingest chunks of data via Streams
4. Take advantage of zero-copying whenever possible, possibly creating the missing machinery
5. Expose the appropriate maintenance required by the underlying engine
5. Have detailed benchmarks for performance comparisons, which should support:
  * UI-triggered benchmarks
  * Criterion benchmarks that compare backends in a quiet, repeatable way.

### Database Models
1. For mmap-able records -- enabling zero-copy on reads and writes for suitable database engines:
   To use the mmap features of the available database engines, the database data model should explicitly honor the storage layout contract:
  1. Use `#[repr(C)]` when bytes are interpreted as a Rust struct.
  2. Use `bytemuck::Pod` / `bytemuck::Zeroable` only when the type truly satisfies their invariants.
  3. Add explicit padding fields when needed to preserve alignment and fixed-size layout.