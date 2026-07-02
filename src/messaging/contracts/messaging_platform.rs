/// All Messaging Platforms that this module has implementations for.
/// See [super::super::gateways]
#[derive(Debug, PartialEq)]
pub enum MessagingPlatform {
    Telegram,
    Whatsapp,
    Slack,
    MicrosoftTeams,
    /// Used by unit tests
    TestPlatform,
}
