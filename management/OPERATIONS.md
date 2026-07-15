# Operational Environment "OpEnv"

This section describes the patterns to use on Production, Stage, Testing, and whatever shared environments we have.
Due to the nature of "whole system into a single machine" approach -- containing the application, config, media, database, etc. –
we'll be using small long-lived VMs – in opposition to short-lived Kubernetes Pods.
The homologated OS for these environments is CachyOS with BTRFS. Developers are strongly advised to use an Archlinux-based distro with the same filesystem.


## 01) The /operations Directory

Everything needed to execute our program – apart from OS libs and default systemd files – will be put under `/operations/OgreRobot.com/`:
1. `bin/`: the program, encrypted/obfuscated config files, and scripts;
2. `certificates`: certificate files and account data. We use the `lego` tool to have free certificates generated;
3. `db/`: where the database files live;
4. `logs/`: rotational log files;
5. `metrics/`: usage reports and so on;
6. `packages/`: where both the current, the previous, and new packages should live.


## 02) The CachyOS Installable Package

From what is described above in `OpEnv.01`, releases should be made available via `pacman` installable packages. The package should:
1. Be created with the appropriate version specification and target environment suffix – either `-prod`, `-stage`, `-test`, `-dev`, etc.
2. The package should always provide the full contents of the `bin/` directory, except for the config file – for which we should provide a default & unobfuscated one with the suffix `.example`;
3. The full set of `systemd` files – services, timers, etc. – to be placed on the system directory – allowing the user to overwrite anyone of them in the usual `/etc` way;
4. Any one-time scripts – such as database or config migrations – to be activated immediately after updates, creating backups via BTRFS snapshots;
5. Among the `systemd` & scripts, a simple "Snapshot Management System" is included, described next in `OpEnv.02.a`;
6. Also, a `rollback` script is provided, described in `OpEnv.02.b`;
7. All contents from the package must come from the git repo, under `/operations/OgreRobot.com/`.



# Data Storage "DS"

This section defines some rules around data storage, balancing performance, maintainability, and endurance.


## 01) Types of Persistent Data

The BOT makes uses the following types of persistence:

1. Application Config files: some command lines may cause the configs to be changed and saved. The file needs to be obfuscated, as it holds sensitive information.
2. Session Data: this is, in essence, data that lives in RAM needed to correctly conduct the user along each dialog and conversation state. In practice, this data is persisted because we want that any application
   deployments or crashes to go unnoticed; Also, the BOT may decide to clear from RAM some dialogs after an inactivity period – which should be transparently restored.
3. Formally Persisted Data: this is the data that we cannot lose, otherwise it would be perceived as a degradation of the service or even would drain our ability from receiving revenue. Examples are: user profile data,
   preferences, any integration or progress details, and so on.

Note: to disambiguate further between Session Data and Formally Persisted Data: we may discard Session Data in certain occasions, without losing any service quality:
1. When the user says "goodbye" – the session may be deleted.
2. The user did not show up for an extended period of time – the session may be deleted and a message saying "Welcome back" is presented to emphasize we are starting fresh.
 

### 01.a) Session Data Storage

No special constraints is imposed on what backend we may use for Session Data, other than we need to persist it so application restarts are supported transparently, from the user perspective 

## 02) Formally Persisted Data:

For this kind of data, we want a storage backend that allows the following:
1. It should allow inspecting the model – for both automated and manual evaluation
2. It should allow changing the model – both automated or manually
3. A human should be aboe to inspect the current data at any time – including when the BOT is executing

For easy referencing, when we mention "Database" throughout the requirements and backlogs, we are referring to the platform, backend, or location for this kind of data.