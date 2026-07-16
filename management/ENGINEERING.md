# Messaging Communications Paradigm "MCP"

The BOT communicates with the Messaging Platform defined in [BUSINESS/Messaging Platform/01 Supported Messaging Platforms] in a reactive & functional way
-- all Messaging Platforms comes in via a MO Stream and should leave via an MT Stream -- where MO stands for Mobile Originated and MT stands for Mobile Terminated.


## 01) Per User Routing

From the dialog handling logic point of view, each user should receive its own Stream of MOs and should hand over the Stream of MTs. This routing logic is handled by each Messaging Platform gateway.
Should any messages cause an unknown error, it should be presented to the user in addition to being logged.
No errors should disrupt the underlying Messaging Platform instance, the Per-User Routing, nor the current user dialog processor logic. 


## 02) Messaging Platform Agnosticism

The dialog handling logic may or may not care about the Messaging Platform through which the communications are running.


## 02.a) Common MO and MT Models

To allow a single logic unit to support multiple Messaging Platforms, a common "MO", "MT", and "User" models are defined.
Any breaking upstream changes to the Messaging Platform should be identified at any of the following steps – the sooner the better:
1. Program failing to compile;
2. Program failing to execute – quitting with a clear error message regarding model drift;
3. Messaging Platform stopping to execute – with a log message clearly stating the model drift;
4. QA session via one of the test plans.


## 02.b) Messaging Platform Inquiry and Features Inquiry

If the logic wants to split itself – e.g., one logic for Slack and another logic for SMS – it can do so by:

1. Inquiring the Underlying Messaging Platform
2. Inquiring the Features Available – for instance, message edition, picture support, etc.

We will not provide a full list of features here, as each Messaging Platform is the source of truth.
Note: [BUSINESS/Messaging Platforms/02 Supported Features] define how we should monitor and follow new features as they are made available for each Messaging Platform.



# Software Design Architecture for the User-Facing Functionalities "FuncArch"

This section defines the main software architecture paradigm we use in this BOT in regard to functionalities perceived by the users.
The architecture defines and work on the following principles:
* How new services will hook to pre-existing logic
* Each functionality should be decoupled from the others – except for the "base functionality"
* The "base functionality" consists of the basic "User Management" and "Session" features – responsible for creating, querying, deleting users, and keeping "Session Data" – detailed bellow.


## 01) The Microservices Architecture

Microservices are usually discussed as an operational/deployment architecture. But we are reusing many of its underlying design principles, which are made possible by the use of Streams and
"Per User Routing" described in this document. Especially, we care about:
* internal features to be decoupled
* composable
* independently understandable
* and connected through clear contracts.

We are calling this the "Micro-Composable Architecture" (containing "Micro-components") and has consequences on how the project is organized.
It is now time to define it.

PS: This architecture might eventually become a real distributed Microservices architecture, where each component might be deployed and scaled independently.


### 01.a) Project organization with respect to the "Micro-Composable Architecture"

Each fine-grained functionality lives in `/src/micro`. Let's take the example of the "User Profile" `micro-component` -- which contain additional user info and config:
* Lives in `/src/micro-component/user_profile`;
* Contains the submodules `logic`, `repository`, and `sessions.rs`;
* An entry in `SessionEntry` enum -- in this example, `UserProfle(UserProfileSession),`;
* May use any code from the project outside the `micro-component` module, but may only use code from other micro-components through `logic/contracts`
  -- this guarantees each micro-component to be independently testable and provide a controlled boundary between them for improved maintainability.

Next, the formal definition of the related components and micro-components that are available.


### 01.b) The baseline "User Management" infrastructure

This layer is basic and responsible for handling "User IDs" – be it a username or mobile phone id, depending on the Messaging Platform. It will, simply:
* Create an entry for that user/platform in the database;
* Map any external ID to our Internal ID, so other micro-components may reference them;
* Will provide a way for other micro-components to "register themselves" into that user;
* Will handle user deletion – calling each registered micro-component "Delete User" operation.

This module, together with the "Sessions" one, are the only ones in this list that lives outside the `/src/micro` path. Think of them as the necessary bind to all micro-components and, since they has
special characteristics, don't belong alongside them.


### 01.c) The baseline "Session" infrastructure

This layer is responsible for managing all runtime data (a.k.a., Session Data) used by the micro-components with the following characteristics:
* Session Data is automatically loaded when the Dialog Processor starts and persisted when it ends;
* It contains a single structure: `HashSet<SessionEntry>`;
* `SessionEntry`, as mentioned before, is an enum that contains a single tuple variant for every micro-component -- receiving a single unnamed parameter 


### 01.d) The "User Profile" micro-component: `user_profile`

Allows recording additional information to the given User ID (from the baseline "User Management" infrastructure). To be decoupled, this is done in its own table
-- as with every entry in the `/src/micro-component/` module -- and provide consultation to other micro-compinents via the exposed contracts (having these implementations
in `user_profile/logic/domain`), and also provide a dialog interface to gather (and show) such information (as usual, implemented in `user_profile/logic/bot`).


### 01.e) The "i18n" infrastructure

This layer is responsible for managing the contents of "Internationalization" -- or, more specifically, providing different versions for the following content:
* Texts
* Images
* Command interpretation.

And it may operate by distinguishing over several dimensions:
* language – the user preferred locale
* interface – the Messaging Platform
* age – to allow controlling the tone.



# Demoscenes "Demo"

In order to support what is specified in the sessions of [BUSINESS/Messaging Platforms], we use "Demoscene Examples". Those are:
1. Entries in Rust's `/examples` directory;
2. Should not make use of our `messaging` layer -- see bellow -- as we need total freedom;
3. These examples are the driver of the refactorings in `messaging` layer
4. Each application in `/examples` should correspond to a single Messaging Platform
5. Each Messaging Platform should have only one entry in `/examples`


## 01) Telegram

An example program to demoscene the currently documented official Telegram features.
The example should contain docs on how to create the Telegram Account (a.k.a., Telegram Bot Entry) and have it configured in the example 
so that local developers wanting to execute this example should be able to easily follow the necessary procedures.
Even if it is just an example, the program should not hide errors but may simply exit should one happen. 

## 02) WhatsApp

An example program to demoscene the currently documented official WhatsApp features.
The example should contain docs on how to create the WhatsApp Development Account and have it configured in the example
so that local developers wanting to execute this example should be able to easily follow the necessary procedures.
Even if it is just an example, the program should not hide errors but may simply exit should one happen.



# Architecture "Arch"

The BOT's program is organized into layers of responsibility, implemented via Rust (sub) modules. Additionally, we use dependency inversion to ease testability.


## 01) `messaging` Layer

This layer defines the common models as well as the contracts and implementations to handle all the "Supported Messaging Platforms".


## 02) `db` Layer

Here lives, all façade, wrappers, and helpers related to Data Persistence -- the exception being the "Application Config" infrastructure.
See the definitions in [OPERATIONS/Data Storage/01 Types of Persistent Data].


## 03) Logic Layer

This is where the soul of the BOT lives; what brings the users value; what controls how to respond to each input or when to start a dialog.

In accordance with [BUSINESS/Messaging Platforms/03 Additional "Virtual" Messaging Platforms], the logic layer, itself, has sub-layers:
* `logic/bot` -- drives the dialogs, states, and general messaging communications;
* `logic/domain` -- owns querying rules, stored information, validation, constraints, external integrations. This layer is accessible via external APIs.



# Documentation "Docs"

All software modules should use Doc Comments wherever applicable, with references to related parts for further clarity.

The documentation should tell what the code usually doesn't tell:
* Why this entity exists / why it is necessary – e.g., "This module manages communications with Telegram"
* How does it fit in the rest of the software – e.g., "It uses the [UserRouter] facility to allow a unique Dialog Processor for each user".

Since we are building an executable entity, the ocs' primarily goal is to help the Engineers working with this software to be in sync and to get to know all the available components
-- instead of libraries, in which the main docs purpose would be to ease external teams integration.

Additionally:


## 01) Docs as part of the CI process

We must publish the current docs in a site, so it can be readily consulted.


## 02) Documentation of Private / Internal items

Since our goal is to ease the local team, it is of utter importance documenting private items as well as unit and integration tests.



# Repository Governance Automation "Gov"

## 01) Management Governance System

The repo-local management system must preserve human authority over requirements while making the documented process safe, repeatable, and auditable. The management command line, local authoring console, static report, and AI workflow must share one coherent interpretation of requirements, work items, evidence, and supporting ledgers.

It has three coordinated obligations:

1. Deterministic integrity: the system must validate the complete management graph, including requirement/work-item grammar, state history, blockers, supersession, traceability, and supporting-ledger structure. Management-record mutations must preserve unrelated text, including history; reject repository-escaping record paths or evidence paths; serialize competing writers; replace files atomically. Invalid input or a failed multi-record update must leave the repository in its prior coherent state. State changes must follow the documented transition rules, retain every dated state already reached, and distinguish lifecycle state from the independently recorded blocked condition.
2. Shared authoring surfaces: the local GUI and static Management Report must continue to use a shared model/frontend. Static mode must expose the full workflow surface without enabling local actions. Local authoring must bind to loopback by default; non-loopback binding requires an explicit unsafe acknowledgement plus a host allowlist. Actions must have cross-origin protection, mutation serialization, and in-page repeat-submission protection. Reports plus diagrams must be coherent snapshots, use checkout-specific temporary storage, and avoid partial or stale publication.
3. Semantic management analysis: deterministic commands must provide explicit evidence in machine-readable signals without presenting lexical heuristics as semantic conclusions. A repo-local AI skill must use those signals together with the governing requirements, backlog, traceability, source code, tests, and relevant git history to perform qualitative plan, verification, review, technical-debt, and requirement-drift analysis. Semantic reports must distinguish observed evidence, inference, and unknowns. The human owner controls requirement changes and final acceptance.
