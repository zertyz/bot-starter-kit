//! Common code -- used by all gateways -- to:
//! 1. route MOs from each messaging platform to each user's processor -- using 1 MO stream per user
//! 2. because of the above, also 1 MT stream per user will be used.
//! 3. this module, then, joins back all per-user MT streams into a single MT stream

use crate::messaging::contracts::messaging::{Mo, Party};
use crate::messaging::contracts::messaging_platform::MessagingPlatform;
use crate::models::config::BotConfig;
use futures::{Stream, StreamExt};
use log::{debug, error, info};
use ogre_stream_ext::{StreamExtCloseOnItemTimeout, throttle_stream};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::future;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct UserRouter<UserType: Clone, HandleType, InnerMoType, InnerMtType> {
    config: BotConfig,
    messaging_platform: MessagingPlatform,
    global_mo_routing_task: Mutex<Option<JoinHandle<()>>>,
    /// The aggregator of per-user channels that will receive each user's MO
    per_user_mo_tx: Arc<Mutex<HashMap<u64, async_channel::Sender<Mo<UserType, InnerMoType>>>>>,
    _phantom: PhantomData<fn(HandleType, InnerMtType)>,
}

impl<UserType: Debug + Clone + Send + Sync + 'static, HandleType: Send + Sync + 'static, InnerMoType: Debug + Send + 'static, InnerMtType: Send + 'static>
    UserRouter<UserType, HandleType, InnerMoType, InnerMtType>
{
    /// Instantiates & spawns a new router task for the given `messaging_platform_mo_stream`
    pub fn new(config: &BotConfig, messaging_platform: MessagingPlatform) -> Self {
        let per_user_mo_tx = Arc::new(Mutex::new(HashMap::new()));

        Self {
            config: config.clone(),
            messaging_platform,
            global_mo_routing_task: Mutex::new(None),
            per_user_mo_tx,
            _phantom: PhantomData,
        }
    }

    pub async fn start<
        HandleSupplierType: MessagingPlatformHandleSupplier<HandleType> + Send + Sync + 'static,
        ProcessorType: UserMoProcessor<UserType, HandleType, InnerMoType, InnerMtType> + Send + Sync + 'static,
    >(
        self,
        messaging_platform_mo_stream: impl Stream<Item = Mo<UserType, InnerMoType>> + Send + 'static,
        handle_supplier: HandleSupplierType,
        per_user_mo_processor: ProcessorType,
    ) -> impl Stream<Item = InnerMtType> + Send + 'static {
        let (messaging_platform_mt_producer, messaging_platform_mt_consumer) = async_channel::bounded(64);

        let instance = Arc::new(self);
        // spawn the global perform MO routing task
        let global_mo_routing_task = tokio::spawn({
            let instance = instance.clone();
            let messaging_platform_mt_consumer = messaging_platform_mt_consumer.clone();
            async move {
                info!("UserRouter: Starting the global MO routing task for {:?}", instance.messaging_platform);
                instance
                    .global_mo_routing_task(messaging_platform_mo_stream, messaging_platform_mt_producer.clone(), handle_supplier, per_user_mo_processor)
                    .await;
                info!("UserRouter: Shutting Down the global MO routing task for {:?}", instance.messaging_platform);
                // wait until all MTs are processed -- up until the shutdown grace period
                tokio::time::sleep(Duration::from_millis(100)).await; // prevents fast starts from missing messages
                let grace_period_start = Instant::now();
                loop {
                    let remaining_mos = instance.per_user_mo_tx.lock().await.values().fold(0, |acc, mo_tx| acc + mo_tx.len());
                    let remaining_mts = messaging_platform_mt_consumer.len();
                    let grace_period_elapsed = grace_period_start.elapsed();
                    if grace_period_elapsed
                        >= instance
                            .config
                            .dialog_processor
                            .shutdown_grace_period
                    {
                        info!(
                            "UserRouter: Shut Down grace period of {:?} is over. Ignoring the remaining {remaining_mos} MOs and {remaining_mts} MTs.",
                            instance
                                .config
                                .dialog_processor
                                .shutdown_grace_period,
                        );
                    } else if remaining_mos + remaining_mts > 0 {
                        info!(
                            "UserRouter: Shutting Down... waiting up to {:?} for {remaining_mos} MOs and {remaining_mts} MTs to be processed upstream...",
                            instance
                                .config
                                .dialog_processor
                                .shutdown_grace_period
                                - grace_period_elapsed
                        );
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                    // close each user's MO channel
                    instance
                        .per_user_mo_tx
                        .lock()
                        .await
                        .values()
                        .for_each(|mo_tx| {
                            mo_tx.close();
                        });
                    // close the global MT channel
                    _ = messaging_platform_mt_consumer.close();
                    _ = messaging_platform_mt_producer.close();
                    break;
                }
            }
        });
        instance
            .global_mo_routing_task
            .lock()
            .await
            .replace(global_mo_routing_task);

        messaging_platform_mt_consumer
    }

    async fn global_mo_routing_task<
        HandleSupplierType: MessagingPlatformHandleSupplier<HandleType> + Send + Sync + 'static,
        ProcessorType: UserMoProcessor<UserType, HandleType, InnerMoType, InnerMtType> + Send + Sync + 'static,
    >(
        self: &Arc<Self>,
        messaging_platform_mo_stream: impl Stream<Item = Mo<UserType, InnerMoType>>,
        messaging_platform_mt_producer: async_channel::Sender<InnerMtType>,
        handle_supplier: HandleSupplierType,
        per_user_mo_processor: ProcessorType,
    ) {
        let dialog_processor_idle_timeout = self
            .config
            .dialog_processor
            .dialog_processor_idle_timeout;
        let per_user_mo_throttle_interval = self
            .config
            .dialog_processor
            .per_user_mo_throttle_interval;
        let handle_supplier = Arc::new(handle_supplier);
        let per_user_mo_processor = Arc::new(per_user_mo_processor);
        messaging_platform_mo_stream
            .for_each(|mo| {
                debug!("UserRouter: A message arrived through {:?}: {mo:?}", self.messaging_platform);
                let this = self.clone();
                let messaging_platform_mt_producer = messaging_platform_mt_producer.clone();
                let handle_supplier = handle_supplier.clone();
                let per_user_mo_processor = per_user_mo_processor.clone();
                async move {
                    if let Some(new_dialog_context) = this
                        .route_mo(mo)
                        .await
                    {
                        let NewDialogContext { user, mo_tx, mo_rx } = new_dialog_context;
                        let per_user_mo_processor = per_user_mo_processor.clone();
                        tokio::spawn(async move {
                            info!(
                                "UserRouter: Starting Dialog Processor (and MT Routing) tasks for user #{}{}{}",
                                user.id(),
                                user.address
                                    .as_deref()
                                    .map(|address| format!("/{address}"))
                                    .unwrap_or_default(),
                                user.name
                                    .as_deref()
                                    .map(|name| format!(", named '{name}'"))
                                    .unwrap_or_default()
                            );
                            let handle = handle_supplier
                                .supply()
                                .await;
                            let per_user_mo_stream = this.clone().set_close_on_idle(user.id(), mo_tx.clone(), mo_rx, dialog_processor_idle_timeout);
                            let per_user_mo_stream = Self::set_throttle(per_user_mo_stream, per_user_mo_throttle_interval);
                            let user_mt_stream = per_user_mo_processor
                                .process(handle, per_user_mo_stream)
                                .await;
                            // process 1 message at a time (per user)
                            user_mt_stream
                                .for_each(|user_mt| async {
                                    let route_result = messaging_platform_mt_producer
                                        .send(user_mt)
                                        .await;
                                    if let Err(err) = route_result {
                                        error!("Telegram: Bailing out of User's Dialog Processor task: Error routing user_id #{}'s MT message to the all-users mt channel: {err}", user.id());
                                    }
                                })
                                .await;
                            info!(
                                "UserRouter: Dialog Processor (and MT Routing) tasks were closed for user #{}{}{}",
                                user.id(),
                                user.address
                                    .as_deref()
                                    .map(|address| format!("/{address}"))
                                    .unwrap_or_default(),
                                user.name
                                    .as_deref()
                                    .map(|name| format!(", named '{name}'"))
                                    .unwrap_or_default()
                            );
                            this.close_dialog(user.id(), &mo_tx)
                                .await;
                        });
                    }
                }
            })
            .await;
    }

    /// Routes an MO to the per-user processor, signaling the caller if a new dialog processor task should be created (return is `Some`) or one already exists (return is `None` `).
    async fn route_mo(self: &Arc<Self>, mo: Mo<UserType, InnerMoType>) -> Option<NewDialogContext<UserType, InnerMoType>> {
        let user = mo.sender();
        let user_id = user.id();
        let send_mo = async |mo, mo_tx: &async_channel::Sender<Mo<UserType, InnerMoType>>| {
            debug!("Telegram: `route_mo_with_before_new_dialog()`: routing MO");
            mo_tx
                .send(mo)
                .await
                .map_err(|err| {
                    error!("Telegram: Bailing out of User's Dialog Processor task: Error routing MO message to user_id #{user_id}'s channel: {err}");
                    mo_tx.close();
                    err.into_inner()
                })
        };

        /// Routing decision after the dialog map has been updated.
        enum DialogRoute<UserType: Clone, InnerMoType> {
            Existing(async_channel::Sender<Mo<UserType, InnerMoType>>),
            New {
                mo_tx: async_channel::Sender<Mo<UserType, InnerMoType>>,
                mo_rx: async_channel::Receiver<Mo<UserType, InnerMoType>>,
            },
        }

        let route = {
            let mut locked_dialog_mo_tx_by_user = self
                .per_user_mo_tx
                .lock()
                .await;
            match locked_dialog_mo_tx_by_user.entry(user_id) {
                Entry::Occupied(entry) => {
                    // A channel already exists for the user. Simply route the message.
                    DialogRoute::Existing(
                        entry
                            .get()
                            .clone(),
                    )
                }
                Entry::Vacant(entry) => {
                    // No channel exists yet for the user. Create one, Store & spawn the Dialog Processor task... and also send the message as above
                    let (mo_tx, mo_rx) = async_channel::unbounded();
                    entry.insert(mo_tx.clone());
                    DialogRoute::New { mo_tx, mo_rx }
                }
            }
        };

        match route {
            DialogRoute::Existing(mo_tx) => {
                _ = send_mo(mo, &mo_tx).await;
                None
            }
            DialogRoute::New { mo_tx, mo_rx } => {
                let user = user.clone();
                _ = send_mo(mo, &mo_tx).await;
                Some(NewDialogContext { user, mo_tx, mo_rx })
            }
        }
    }

    /// Upgrades the `mo_stream` to one that auto-closes after no MO arrives within `dialog_idle_timeout`.
    fn set_close_on_idle(
        self: Arc<Self>,
        user_id: u64,
        mo_tx: async_channel::Sender<Mo<UserType, InnerMoType>>,
        mo_stream: impl Stream<Item = Mo<UserType, InnerMoType>> + Send + 'static,
        dialog_idle_timeout: Duration,
    ) -> impl Stream<Item = Mo<UserType, InnerMoType>> + Send {
        mo_stream
            .boxed()
            .close_stream_on_item_timeout(dialog_idle_timeout)
            .then(move |mo_timeout_result| {
                let this = self.clone();
                let mo_tx = mo_tx.clone();
                async move {
                    match mo_timeout_result {
                        Ok(mo) => Some(mo),
                        Err(_err) => {
                            let removed = this
                                .close_dialog(user_id, &mo_tx)
                                .await;
                            info!(
                            "UserRouter: Closing dialog processor for user #{user_id} after {dialog_idle_timeout:?} without receiving MOs{}",
                            if !removed { " -- BUT IT SEEMS TO HAVE BEEN REMOVED ALREADY" } else { "" }
                        );
                            None
                        }
                    }
                }
            })
            .filter_map(future::ready)
    }

    /// Upgrades the `mo_stream` to yield MOs at most once per `throttle_interval`.
    fn set_throttle(mo_stream: impl Stream<Item = Mo<UserType, InnerMoType>> + Send + 'static, throttle_interval: Duration) -> impl Stream<Item = Mo<UserType, InnerMoType>> + Send {
        if throttle_interval.is_zero() {
            mo_stream.boxed()
        } else {
            let elements_per_second = 1.0 / throttle_interval.as_secs_f64();
            throttle_stream(mo_stream, elements_per_second).boxed()
        }
    }

    /// Removes a dialog route before closing its channel.
    async fn close_dialog(self: &Arc<Self>, user_id: u64, mo_tx: &async_channel::Sender<Mo<UserType, InnerMoType>>) -> bool {
        let had_dialog_route = self
            .per_user_mo_tx
            .lock()
            .await
            .remove(&user_id)
            .is_some();
        if had_dialog_route {
            mo_tx.close();
        }
        had_dialog_route
    }
}

/// per user Stream processor
pub trait UserMoProcessor<UserType: Clone, HandleType, InnerMoType, InnerMtType> {
    /// per user Stream processor
    fn process<MoStream: Stream<Item = Mo<UserType, InnerMoType>> + Send>(&self, handle: HandleType, user_mo_stream: MoStream) -> impl Future<Output = impl Stream<Item = InnerMtType> + Send> + Send;
}

/// per Messaging Platform handle supplier (e.g., Teloxide's `Bot` handle)
pub trait MessagingPlatformHandleSupplier<HandleType> {
    /// per Messaging Platform handle supplier (e.g., Teloxide's `Bot` handle)
    fn supply(&self) -> impl Future<Output = HandleType> + Send;
}

/// Context needed to spawn a processor for a newly created dialog between this bot and a user.
struct NewDialogContext<UserType: Clone, InnerMoType> {
    user: Party<UserType>,
    mo_tx: async_channel::Sender<Mo<UserType, InnerMoType>>,
    mo_rx: async_channel::Receiver<Mo<UserType, InnerMoType>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::contracts::messaging::{Dialog, DialogKind, Language};
    use futures::{StreamExt, stream};
    use teloxide::types::{User, UserId};
    use tokio::pin;
    use tokio::time::{sleep, timeout};

    /// Assures the low-level router behavior: new Dialog Processor creation is commanded on the first (and only on the first) message a user sends
    #[tokio::test]
    async fn routing_on_first_user_message() {
        const USER_ID: u64 = 97;

        let instance = UserRouter::<_, (), (), ()>::new(&TEST_CONFIG, MessagingPlatform::TestPlatform);
        let instance = Arc::new(instance);
        let first_routing = instance
            .route_mo(test_mo(USER_ID, 1))
            .await;
        assert!(first_routing.is_some(), "The first MO should provoke a new Dialog creation");
        let second_routing = instance
            .route_mo(test_mo(USER_ID, 1))
            .await;
        assert!(second_routing.is_none(), "After the first MO, no new Dialogs should be created");
    }

    /// Confirms the high level behavior of a User's first message: it should place an entry on the Currently on-going Dialog Processors list;
    /// unless the Dialog had been terminated -- in which case, a further message will, again, be considered the first one.
    #[tokio::test]
    async fn dialogs_on_first_user_message() {
        const USER_ID: u64 = 97;
        const TIMEOUT: Duration = Duration::from_millis(150);

        let (platform_mo_producer, platform_mo_stream) = async_channel::bounded(4);

        let instance = UserRouter::new(&TEST_CONFIG, MessagingPlatform::TestPlatform);
        let per_user_mo_tx = instance
            .per_user_mo_tx
            .clone();
        let platform_mt_stream = instance
            .start(platform_mo_stream, TestHandleSupplier, TestMoProcessor)
            .await;
        pin!(platform_mt_stream);

        let mut assert_first_message_dialog_creation = async || {
            platform_mo_producer
                .send(test_mo(USER_ID, 1))
                .await
                .expect("Couldn't send an MO");
            // wait for the answer
            let first_mt_result = timeout(TIMEOUT, platform_mt_stream.next()).await;
            match first_mt_result {
                Ok(first_mt) => assert!(first_mt.is_some(), "`UserRouter` was closed before the 1st MT arrived"),
                Err(_err) => panic!("Timeout while waiting for the 1st MT"),
            };
            // check
            let contains_user_dialog = per_user_mo_tx
                .lock()
                .await
                .contains_key(&USER_ID);
            assert!(contains_user_dialog, "The User Dialog Processor task wasn't created on the First Message")
        };

        // publish the first MO
        assert_first_message_dialog_creation().await;
        // artificially close the dialog
        per_user_mo_tx
            .lock()
            .await
            .get(&USER_ID)
            .map(async_channel::Sender::close);
        // additionally check that closed channels do remove the entry from the map
        sleep(Duration::from_millis(1)).await;
        let contains_user_dialog = per_user_mo_tx
            .lock()
            .await
            .contains_key(&USER_ID);
        assert!(!contains_user_dialog, "The User Dialog Processor task entry wasn't removed after the stream had finished");
        // publish a first MO again -- should reopen the dialog processor
        assert_first_message_dialog_creation().await;
    }

    /// Makes sure our routing is rock-solid by simulating a surge of messages from the same user.
    /// We want to make sure only one dialog processor per user will be created.
    #[tokio::test]
    async fn single_user_concurrency() {
        const ROUTED_MESSAGES_COUNT: u64 = 128;
        const USER_ID: u64 = 97;
        const TIMEOUT: Duration = Duration::from_millis(150);

        let mo_iter = (0..ROUTED_MESSAGES_COUNT).map(|i| test_mo(USER_ID, i));
        let mo_stream = stream::iter(mo_iter);

        let instance = UserRouter::new(&TEST_CONFIG, MessagingPlatform::TestPlatform);
        let mt_stream = instance
            .start(mo_stream, TestHandleSupplier, TestMoProcessor)
            .await;
        pin!(mt_stream);

        for i in 0..ROUTED_MESSAGES_COUNT {
            let mt_result = timeout(TIMEOUT, mt_stream.next()).await;
            let mt = match mt_result {
                Ok(Some(mt)) => mt,
                Ok(None) => panic!("`UserRouter` was closed before the MT #{i} arrived"),
                Err(_err) => panic!("Timeout while waiting for the MT #{i}"),
            };
            // Since MT messages produced by `test_mo()` do contain #{message_id} in them:
            assert!(mt.contains(&format!(" #{i} ")), "Failed at checking MT #{i}")
        }

        // assert the end of the MO stream also triggers the end of the MT stream
        let mt_result = timeout(TIMEOUT, mt_stream.next()).await;
        match mt_result {
            Ok(mt) => assert!(mt.is_none(), "It seems the MT stream contains additional (unexpected) messages"),
            Err(_err) => panic!("Timeout while for the MT stream to be closed"),
        };
    }

    /// Assures the per-user MO stream is throttled before messages reach the Dialog Processor.
    #[tokio::test]
    async fn per_user_mo_stream_throttle() {
        const EXPECTED_MESSAGES_COUNT: u64 = 3;
        const USER_ID: u64 = 97;
        const THROTTLE_INTERVAL: Duration = Duration::from_millis(30);
        const TIMEOUT: Duration = Duration::from_secs(5);

        let mut config = TEST_CONFIG;
        config
            .dialog_processor
            .per_user_mo_throttle_interval = THROTTLE_INTERVAL;

        let mo_stream = stream::iter((0..EXPECTED_MESSAGES_COUNT)
            .map(|i| test_mo(USER_ID, i)));
        let instance = UserRouter::new(&config, MessagingPlatform::TestPlatform);
        let mt_stream = instance
            .start(mo_stream, TestHandleSupplier, TestMoProcessor)
            .await;

        let minimum_duration = Duration::from_secs_f64(THROTTLE_INTERVAL.as_secs_f64() * (EXPECTED_MESSAGES_COUNT as f64 - 1.0));
        let start = Instant::now();
        let observed_mts_count = timeout(TIMEOUT, mt_stream
            .count()).await
            .expect("Timeout while waiting for the MTs");
        assert_eq!(observed_mts_count as u64, EXPECTED_MESSAGES_COUNT, "Number of MTs do not match");
        assert!(start.elapsed() >= minimum_duration, "The per-user MO stream was not throttled");
    }

    /// Assures the shutdown grace period is honored before hard closing the router
    #[tokio::test]
    async fn grace_period() {
        const USER_ID: u64 = 97;

        let expected_count = 4;

        let instance = UserRouter::new(&TEST_CONFIG, MessagingPlatform::TestPlatform);
        let mo_stream = stream::iter((0..expected_count).map(|i| test_mo(USER_ID, i)));
        let mt_stream = instance
            .start(mo_stream, TestHandleSupplier, TestMoProcessor)
            .await;

        // slow MT processor -- MTs have long been issued but are taking some time to process
        let mut observed_count = 0;
        mt_stream
            .inspect(|_mt| observed_count += 1)
            .for_each(|_mt| sleep(Duration::from_secs(1)))
            .await;

        assert_eq!(observed_count, expected_count, "not all MTs arrived");
    }

    /// Makes sure idle dialog processors are removed from the routing table.
    #[tokio::test]
    async fn idle_dialog_timeout() {
        const USER_ID: u64 = 97;
        const IDLE_TIMEOUT: Duration = Duration::from_millis(100);

        // custom config for this test
        let mut config = TEST_CONFIG;
        config
            .dialog_processor
            .dialog_processor_idle_timeout = IDLE_TIMEOUT;

        let (mo_producer, mo_stream) = async_channel::bounded(4);

        let instance = UserRouter::new(&config, MessagingPlatform::TestPlatform);
        let per_user_mo_tx = instance
            .per_user_mo_tx
            .clone();
        let _mt_stream = instance
            .start(mo_stream, TestHandleSupplier, TestMoProcessor)
            .await;

        // send the first message to open the dialog processor
        mo_producer
            .send(test_mo(USER_ID, 1))
            .await
            .expect("Couldn't send an MO");

        // we send no message after that -- the dialog processor should be closed after the IDLE_TIMEOUT
        sleep(IDLE_TIMEOUT).await;
        sleep(Duration::from_millis(10)).await; // wait a little bit more to make sure all proceedings completed

        let contains_user_dialog = per_user_mo_tx
            .lock()
            .await
            .contains_key(&USER_ID);
        assert!(!contains_user_dialog, "The idle users' dialog processor session wasn't terminated after the idle timeout of {IDLE_TIMEOUT:?}");
    }

    const TEST_CONFIG: BotConfig = {
        let mut config = BotConfig::const_default();
        config
            .dialog_processor
            .shutdown_grace_period = Duration::from_mins(1);
        config
            .dialog_processor
            .per_user_mo_throttle_interval = Duration::ZERO;
        config
    };

    fn test_user(user_id: u64) -> User {
        User {
            id: UserId(user_id),
            is_bot: false,
            first_name: format!("test-user-{user_id}"),
            last_name: None,
            username: None,
            language_code: None,
            is_premium: false,
            added_to_attachment_menu: false,
        }
    }

    fn test_mo(user_id: u64, message_id: u64) -> Mo<User, ()> {
        Mo::new(message_id, Party::new(user_id, test_user(user_id)), Dialog::new(777, DialogKind::Private, Language::English), ())
    }

    struct TestHandleSupplier;
    impl MessagingPlatformHandleSupplier<()> for TestHandleSupplier {
        async fn supply(&self) -> () {
            // noop
        }
    }

    struct TestMoProcessor;
    impl UserMoProcessor<User, (), (), String> for TestMoProcessor {
        async fn process<MoStream: Stream<Item = Mo<User, ()>> + Send>(&self, _handle: (), user_mo_stream: MoStream) -> impl Stream<Item = String> {
            user_mo_stream
                .inspect(|mo| println!("<<< MO: {mo:?} "))
                .enumerate()
                // maps the MO to an MT
                .map(|(i, _mo)| format!("response #{i} was produced"))
                .inspect(|mt| println!(">>> MT: {mt}"))
        }
    }
}
