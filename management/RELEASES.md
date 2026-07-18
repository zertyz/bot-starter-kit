# Releases

Production releases are recorded here when a production tag is created. Release-candidate tags are test artifacts and should only be recorded when they affect a release decision.

Use `scripts/management/record_release --help` after the release outcome is known. The command writes the structure below; it does not create or push tags, deploy, link evidence, or advance work items.


## No Production Releases Recorded Yet

The repository currently has no git tags recorded locally.


## Release Entry Format

### `<version>` -- `<release date>`

Tag:
`<semver tag>`

Included work:
1. `<work item id>` -- `<summary>`

Requirements affected:
1. `<requirement id>` -- `<summary>`

Verification:
1. Branch CI:
2. Main CI:
3. Post-merge QA:
4. Operational checks:

Production decision:
`Released`, `Rejected`, or `Rolled back`

Rollback or mitigation:
`<how to revert or reduce impact>`

Notes:
`<short release notes>`
