# Messaging Platforms "MsgP"

Messaging Platforms are the external service, technology, or product through which this BOT communicates with users.
Bellow, we define the Messaging Platforms we support.


## 01) Supported Messaging Platforms

The BOT is able to communicate – to receive MOs and send MTs – with:
 * Telegram
 * WhatsApp
 * Slack
 * Microsoft Teams
 * Discord


## 02) Supported Features

The BOT must support all the features each Messaging Platform provides apart from the obvious "text exchanging".
For instance, Whatsapp allows sending and receiving "GPS Locations"; Telegram allows "Inline Menus".

It is pointless to enumerate all features for all platforms in this document, as this is an ever-growing list and makes no sense to duplicate information here.


### 02.a) Feature Followup

The BOT must be up-to-date with whatever new features each platform support. Let's establish the deadline of 3 months for new features to be made available


## 03) Additional "Virtual" Messaging Platforms

In order to allow the use of Web UI and allow external integration with other systems, we should also support external APIs to access parts of the BOT logic.

APIs, such as:
1. HTTPS
2. Socket-based

Should be able to:
1. List users, sorted by sent message time
2. Fetch all "Formally Persisted Data" (as defined in [OPERATIONS/Data Storage/02 Formally Persisted Data])
3. Make changes to a user "Formally Persisted Data"



# User Management "UsrMgn"

This section defines how we should deal with users, the information needed to start, and we collect, and what is the user lifecycle.


## 01) Valuable Data

This section describes the data the BOT deems valuable and needs to be persisted, as per the definition in [OPERATIONS/Data Storage/02 Formally Persisted Data]