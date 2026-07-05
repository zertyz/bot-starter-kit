# Messaging Communications Paradigm "MCP"

The BOT communicates with the Messaging Platform defined in [BUSINESS/Messaging Platform/01 Supported Messaging Platforms] in a reactive & functional way
-- all Messaging Platforms comes in via a MO Stream and should leave via an MT Stream -- where MO stands for Mobile Originated and MT stands for Mobile Terminated.


## 01) Per User Routing

From the dialog handling logic point of view, each user should receive its own Stream of MOs and should hands over the Stream of MTs. This routing logic is handled by each Messaging Platform gateway implementation.


## 02) Messaging Platform Agnosticism

The dialog handling logic may or may not care about the Messaging Platform through which the communications are running.


## 02.a) Common MO and MT Models

To allow a single logic implementation to support multiple Messaging Platforms, a common MO, MT, and User models are defined.


## 02.b) Messaging Platform Inquiry and Features Inquiry

If the logic wants to split itself -- e.g., one logic for Slack and another logic for SMS -- it can do so by:

1. Inquiring the Underlying Messaging Platform
2. Inquiring the Features Available -- for instance, message edition, picture support, etc.

We will not provide a full list of features here, as each Messaging Platform is the source of truth.
Note: [BUSINESS/Messaging Platforms/02 Supported Features] define how we should monitor and follow new features as they are made available for each Messaging Platform.



# Demoscenes "Demo"

In order to support what is specified in the sessions of [BUSINESS/Messaging Platforms], we use "Demoscene Examples". Those are:
1. Entries in Rust's `/examples` directory;
2. Should not make use of our `messaging` layer -- see bellow -- as we need total freedom;
3. These examples are the driver of the refactorings in `messaging` layer
4. Each application in `/examples` should correspond to a single Messaging Platform
5. Each Messaging Platform should have only one entry in `/examples`


## 01) Telegram

## 02) Whatsapp



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
