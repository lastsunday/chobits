use std::{collections::HashMap, error::Error, sync::Arc, time::Duration};

use futures_util::future::{join, join_all};
use ruma::{
    OwnedRoomId, OwnedUserId, UserId,
    api::client::{
        filter::FilterDefinition, membership::join_room_by_id, message::send_message_event,
        sync::sync_events,
    },
    assign,
    events::{
        AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncMessageLikeEvent,
        room::message::MessageType,
    },
    presence::PresenceState,
    serde::Raw,
};
use ruma_client::DefaultConstructibleHttpClient as _;
use service::chobits::message::listen::{ListenMessage, ListenMode, ListenState};
use tokio::sync::Mutex;
use tokio_stream::StreamExt as _;
use tracing::{error, info};

use crate::{
    config::{
        audio::AudioConfig, matrix::MatrixConfig, mcp::McpConfig, session::SessionConfig,
        vad::VadConfig,
    },
    ws::{frame::Frame, session::Session},
};

pub async fn start(
    matrix_config: Arc<MatrixConfig>,
    session_config: Arc<SessionConfig>,
    mcp_config: Arc<McpConfig>,
    vad_config: Arc<VadConfig>,
    audio_config: Arc<AudioConfig>,
) -> Result<(), Box<dyn Error>> {
    let bot = Bot::build(
        matrix_config,
        session_config,
        mcp_config,
        vad_config,
        audio_config,
    )
    .await?;
    bot.run().await?;
    Ok(())
}

type HttpClient = ruma_client::http_client::HyperNativeTls;
type MatrixClient = ruma_client::Client<HttpClient>;

/// The bot.
struct Bot {
    /// The client to use to make requests against the Matrix API.
    matrix_client: MatrixClient,
    /// The user ID of the Matrix account used by the bot.
    user_id: OwnedUserId,
    session_map: HashMap<String, Arc<Mutex<Session>>>,
    session_config: Arc<SessionConfig>,
    mcp_config: Arc<McpConfig>,
    vad_config: Arc<VadConfig>,
    audio_config: Arc<AudioConfig>,
}

impl Bot {
    /// Build the `Bot` from the config.
    // TODO: session token save and reuse
    async fn build(
        matrix_config: Arc<MatrixConfig>,
        session_config: Arc<SessionConfig>,
        mcp_config: Arc<McpConfig>,
        vad_config: Arc<VadConfig>,
        audio_config: Arc<AudioConfig>,
    ) -> Result<Self, Box<dyn Error>> {
        let matrix_client = ruma_client::Client::builder()
            .homeserver_url(
                matrix_config
                    .homeserver
                    .clone()
                    .expect("matrix homeserver is empty"),
            )
            .build::<HttpClient>()
            .await?;
        let username = matrix_config
            .client_username
            .clone()
            .expect("matrix client username is empty");
        matrix_client
            .log_in(
                &username,
                &matrix_config
                    .client_password
                    .clone()
                    .expect("matrix client password is empty"),
                None,
                matrix_config.client_name.as_deref(),
            )
            .await?;
        let user_id = UserId::parse(username).expect("invalid matrix user id");
        Ok(Self {
            matrix_client,
            session_config,
            mcp_config,
            vad_config,
            audio_config,
            user_id,
            session_map: HashMap::new(),
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
        // create session
        let session_key = &room_id.to_string();
        if !self.session_map.contains_key(session_key) {
            // TODO: init session
            // TODO: send hello frame
            // TODO: recv hello result frame
            // let mut output = session.output_frame().await;

            // TODO: start frame listener async task
            let matrix_client = self.matrix_client.clone();
            tokio::spawn(async move {
                let client = matrix_client;
                let id = room_id;
                // while let Some(data) = output.next().await {
                // let joke_content = RoomMessageEventContent::notice_plain(joke);
                // let txn_id = TransactionId::new();
                // let req = send_message_event::v3::Request::new(
                //     room_id.to_owned(),
                //     txn_id,
                //     &joke_content,
                // )?;
                // // Do nothing if we can't send the message.
                // let _ = self.matrix_client.send_request(req).await;
                // }
            });
            // TODO: add to session map
        }
        let session = self
            .session_map
            .get(session_key)
            .expect("session not exists")
            .clone();
        let mut session = session.lock().await;
        session
            .accept_frame(&Frame::Listen(ListenMessage {
                state: ListenState::Detect,
                mmod: Some(ListenMode::Manual),
                text: Some(&t.body),
                ..Default::default()
            }))
            .await;

        Ok(())
    }

    /// Handle an invitation to the given room.
    async fn handle_invitations(&self, room_id: OwnedRoomId) -> Result<(), Box<dyn Error>> {
        info!("invited to {room_id}");
        self.matrix_client
            .send_request(join_room_by_id::v3::Request::new(room_id.clone()))
            .await?;
        Ok(())
    }
}
