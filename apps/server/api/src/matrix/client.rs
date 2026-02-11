use std::{error::Error, sync::LazyLock, time::Duration};

use futures_util::future::{join, join_all};
use http_body_util::BodyExt as _;
use ruma::{
    OwnedRoomId, OwnedUserId, TransactionId, UserId,
    api::client::{
        filter::FilterDefinition, membership::join_room_by_id, message::send_message_event,
        sync::sync_events,
    },
    assign,
    events::{
        AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent,
        room::message::{MessageType, RoomMessageEventContent},
    },
    presence::PresenceState,
    serde::Raw,
};
use ruma_client::DefaultConstructibleHttpClient as _;
use serde_json::Value as JsonValue;
use tokio_stream::StreamExt as _;
use tracing::{error, info};

use crate::config::{self, matrix::MatrixConfig};

pub async fn start() -> Result<(), Box<dyn Error>> {
    let config = config::get();
    let config = MatrixConfig {
        enable: config.matrix_enable,
        client_name: config.matrix_client_name.clone(),
        homeserver: config.matrix_homeserver.clone(),
        client_username: config.matrix_client_username.clone(),
        client_password: config.matrix_client_password.clone(),
    };
    let bot = Bot::build(config).await?;
    bot.run().await?;
    Ok(())
}

/// The URI used to request a new joke.
static JOKE_API_URI: LazyLock<hyper::Uri> = LazyLock::new(|| {
    "https://v2.jokeapi.dev/joke/Programming,Pun,Misc?safe-mode&type=single"
        .parse()
        .expect("URI should be valid")
});

type HttpClient = ruma_client::http_client::HyperNativeTls;
type MatrixClient = ruma_client::Client<HttpClient>;

/// The bot.
struct Bot {
    /// The client to use to make HTTP requests outside of the Matrix API.
    http_client: HttpClient,
    /// The client to use to make requests against the Matrix API.
    matrix_client: MatrixClient,
    /// The user ID of the Matrix account used by the bot.
    user_id: OwnedUserId,
}

impl Bot {
    /// Build the `Bot` from the config.
    // TODO: session token save and reuse
    async fn build(config: MatrixConfig) -> Result<Self, Box<dyn Error>> {
        let http_client = HttpClient::default();
        let matrix_client = ruma_client::Client::builder()
            .homeserver_url(
                config
                    .homeserver
                    .clone()
                    .expect("matrix homeserver is empty"),
            )
            .build::<HttpClient>()
            .await?;
        let username = config
            .client_username
            .clone()
            .expect("matrix client username is empty");
        matrix_client
            .log_in(
                &username,
                &config
                    .client_password
                    .clone()
                    .expect("matrix client password is empty"),
                None,
                config.client_name.as_deref(),
            )
            .await?;
        let user_id = UserId::parse(username).expect("invalid matrix user id");
        Ok(Self {
            http_client,
            matrix_client,
            user_id,
        })
    }

    /// Run the bot.
    async fn run(&self) -> Result<(), Box<dyn Error>> {
        // Perform an initial sync to ignore messages before the bot was launched.
        let filter = FilterDefinition::ignore_all().into();
        let initial_sync_response = self
            .matrix_client
            .send_request(assign!(sync_events::v3::Request::new(), {
                filter: Some(filter),
            }))
            .await?;

        // Ignore events from our bot.
        let not_senders = vec![self.user_id.clone()];
        let filter = {
            let mut filter = FilterDefinition::empty();
            filter.room.timeline.not_senders = not_senders;
            filter
        }
        .into();

        // Launch a sync loop to listen to messages and invites.
        let mut sync_stream = Box::pin(self.matrix_client.sync(
            Some(filter),
            initial_sync_response.next_batch,
            PresenceState::Online,
            Some(Duration::from_secs(30)),
        ));

        info!("matrix client listening...");
        while let Some(response) = sync_stream.try_next().await? {
            let message_futures =
                response
                    .rooms
                    .join
                    .iter()
                    .map(|(room_id, room_info)| async move {
                        // Use a regular for loop for the messages within one room to handle them sequentially
                        for e in &room_info.timeline.events {
                            if let Err(err) = self.handle_message(e, room_id.to_owned()).await {
                                error!("failed to respond to message: {err}");
                            }
                        }
                    });

            let invite_futures = response.rooms.invite.into_keys().map(|room_id| async move {
                if let Err(err) = self.handle_invitations(room_id.clone()).await {
                    error!("failed to accept invitation for room {room_id}: {err}");
                }
            });

            // Handle messages from different rooms as well as invites concurrently
            join(join_all(message_futures), join_all(invite_futures)).await;
        }

        Ok(())
    }

    /// Handle the given message from the given room.
    async fn handle_message(
        &self,
        ev: &Raw<AnySyncTimelineEvent>,
        room_id: OwnedRoomId,
    ) -> Result<(), Box<dyn Error>> {
        // We are only interested in text messages that contain the word "joke".
        let Ok(AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(
            SyncMessageLikeEvent::Original(m),
        ))) = ev.deserialize()
        else {
            return Ok(());
        };
        let MessageType::Text(t) = m.content.msgtype else {
            return Ok(());
        };

        info!("{}:\t{}", m.sender, t.body);

        if !t.body.to_ascii_lowercase().contains("joke") {
            return Ok(());
        }

        let joke = self
            .get_joke()
            .await
            .unwrap_or_else(|_| "I thought of a joke... but I just forgot it.".to_owned());
        let joke_content = RoomMessageEventContent::notice_plain(joke);

        let txn_id = TransactionId::new();
        let req = send_message_event::v3::Request::new(room_id.to_owned(), txn_id, &joke_content)?;
        // Do nothing if we can't send the message.
        let _ = self.matrix_client.send_request(req).await;

        Ok(())
    }

    /// Handle an invitation to the given room.
    async fn handle_invitations(&self, room_id: OwnedRoomId) -> Result<(), Box<dyn Error>> {
        info!("invited to {room_id}");
        self.matrix_client
            .send_request(join_room_by_id::v3::Request::new(room_id.clone()))
            .await?;

        let greeting = "Hello! My name is Mr. Bot! I like to tell jokes. Like this one: ";
        let joke = self
            .get_joke()
            .await
            .unwrap_or_else(|_| "err... never mind.".to_owned());
        let content = RoomMessageEventContent::notice_plain(format!("{greeting}\n{joke}"));
        let txn_id = TransactionId::new();
        let message = send_message_event::v3::Request::new(room_id, txn_id, &content)?;
        self.matrix_client.send_request(message).await?;
        Ok(())
    }

    /// Get a new joke from the API.
    async fn get_joke(&self) -> Result<String, Box<dyn Error>> {
        let rsp = self.http_client.get(JOKE_API_URI.clone()).await?;
        let bytes = rsp.into_body().collect().await?.to_bytes();

        let joke_obj = serde_json::from_slice::<JsonValue>(&bytes)
            .map_err(|_| "invalid JSON returned from joke API")?;
        let joke = joke_obj["joke"]
            .as_str()
            .ok_or("joke field missing from joke API response")?;

        Ok(joke.to_owned())
    }
}
