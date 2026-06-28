# Code Patterns

This document contains HOW-TO instructions when implementing/extending each of the topics bellow.
This document can be viewed as a lighter version of `CODE_GUIDELINES.md` -- here you'll see guidelines with no enforcement.

## 1. Configuration
All configuration sources, be them:
* Environment variables,
* CLI options,
* Data from external files;
should be unified through the crate `ogre-config-meld`, using the models in `src/models/config.rs` and merged by
`parse_cmdline_and_merge_with_loaded_configs()` in the main module.
1. Should an external crate require an ENV var, we should use `env::set_var()` to allow defining it from the config models 

## 2. Database Wrappers
All database wrappers in `src/db` should:
1. Allow the underlying database to work in async without locking the async workers -- e.g., waiting on a sync mutex to enforce a singlr writer
2. Provide means to receive query results as a Stream, if applicable
3. Provide means to ingest chunks of data via Streams
4. Take advantage of zero-copying whenever possible, possibly creating the missing machinery
5. Expose the appropriate maintenance required by the underlying engine
5. Have detailed benchmarks for performance comparisons

