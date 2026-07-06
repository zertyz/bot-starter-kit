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