impl Session {
    async fn handle_phase_hello<'a>(&mut self, frame: &Frame<'a>) {
        match frame {
            Frame::Hello(hello_message) => {
                let mut has_mcp = false;
                if let Some(features) = &hello_message.features
                    && let Some(mcp) = features.mcp
                {
                    has_mcp = mcp;
                }
                self.handle_connect(hello_message).await;
                self.phase = Phase::ListenDetect;
                if has_mcp {
                    //init Device MCP client
                    let mut mcp_host = self.mcp_host.lock().await;
                    let device_mcp_client = mcp_host
                        .get_device_client()
                        .await
                        .clone()
                        .expect("device mcp not exists");
                    let mut device_mcp_client = device_mcp_client.lock().await;
                    device_mcp_client
                        .request_mcp_initialize(hello_message)
                        .await;
                }
            }
            _ => {
                error!(
                    "invalid frame in phase = {:?},frame = {:?}",
                    self.phase, frame
                );
            }
        }
    }

    async fn handle_phase_listen_detect<'a>(&mut self, frame: &Frame<'a>) {
        match frame {
            Frame::Listen(listen_message) => {
                let state = &listen_message.state;
                match state {
                    ListenState::Start => {
                        let mode = &listen_message.mmod;
                        if let Some(mode) = mode {
                            match mode {
                                service::chobits::message::listen::ListenMode::Auto => {
                                    self.phase = Phase::Listen(ListenMode::Auto);
                                    self.current_mode = RoundMode::Auto;
                                }
                                service::chobits::message::listen::ListenMode::Manual => {
                                    self.phase = Phase::Listen(ListenMode::Manual);
                                    self.current_mode = RoundMode::Manual;
                                }
                                service::chobits::message::listen::ListenMode::RealTime => {
                                    self.phase = Phase::Listen(ListenMode::RealTime);
                                    self.current_mode = RoundMode::RealTime;
                                }
                            }
                            Box::pin(self.accept_frame(frame)).await;
                        } else {
                            error!(
                                "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                                self.phase, frame, state
                            );
                        }
                    }
                    ListenState::Detect => {
                        let mode = &listen_message.mmod;
                        match mode {
                            Some(mode) => {
                                match mode {
                                    service::chobits::message::listen::ListenMode::Auto => {
                                        self.phase = Phase::Listen(ListenMode::Auto);
                                        self.current_mode = RoundMode::Auto;
                                    }
                                    service::chobits::message::listen::ListenMode::Manual => {
                                        self.phase = Phase::Listen(ListenMode::Manual);
                                        self.current_mode = RoundMode::Manual;
                                    }
                                    service::chobits::message::listen::ListenMode::RealTime => {
                                        self.phase = Phase::Listen(ListenMode::RealTime);
                                        self.current_mode = RoundMode::RealTime;
                                    }
                                }
                                Box::pin(self.accept_frame(frame)).await;
                            }
                            None => {
                                // eps32-c3 default listen mode is none
                                // set listen mode to realtime
                                self.phase = Phase::Listen(ListenMode::RealTime);
                                self.current_mode = RoundMode::RealTime;
                                Box::pin(self.accept_frame(frame)).await;
                            }
                        }
                    }
                    _ => {
                        error!(
                            "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                            self.phase, frame, state
                        );
                    }
                }
            }
            Frame::Voice { data } => {
                self.listener.listen(data).await;
            }
            _ => {
                error!(
                    "invalid frame in phase = {:?},frame = {:?}",
                    self.phase, frame
                );
            }
        }
    }

    async fn handle_phase_listen<'a>(&mut self, mode: &ListenMode, frame: &Frame<'a>) {
        match mode {
            ListenMode::Auto => self.handle_phase_listen_for_auto_mode(frame).await,
            ListenMode::Manual => self.handle_phase_listen_for_manual_mode(frame).await,
            ListenMode::RealTime => self.handle_phase_listen_for_realtime_mode(frame).await,
        }
    }

    async fn handle_phase_listen_for_auto_mode<'a>(&mut self, frame: &Frame<'a>) {
        match frame {
            Frame::Listen(listen_message) => {
                let state = &listen_message.state;
                match state {
                    ListenState::Start => {
                        let mode = &listen_message.mmod;
                        if let Some(mode) = mode {
                            match mode {
                                service::chobits::message::listen::ListenMode::Auto => {
                                    self.phase = Phase::Listen(ListenMode::Auto);
                                    self.current_mode = RoundMode::Auto;
                                    self.update_latest_activity_time().await;
                                    self.new_round(RoundMode::Auto).await;
                                    if let Some(round) = &mut self.current_round {
                                        round.accept_command(Command::Wake { text: "Hello" }).await;
                                        let silence_voice_timeout = self
                                            .config
                                            .silence_voice_timeout
                                            .expect("logic silence voice timeout is empty");
                                        //reset listener to option(slinent condition limit)
                                        self.listener.reset(Some(silence_voice_timeout)).await;
                                    } else {
                                        panic!("current round is none");
                                    }
                                }
                                service::chobits::message::listen::ListenMode::Manual => {
                                    self.phase = Phase::Listen(ListenMode::Manual);
                                    self.current_mode = RoundMode::Manual;
                                    self.listener.reset(None).await;
                                }
                                service::chobits::message::listen::ListenMode::RealTime => {
                                    self.phase = Phase::Listen(ListenMode::RealTime);
                                    self.current_mode = RoundMode::RealTime;
                                }
                            }
                        } else {
                            error!(
                                "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                                self.phase, frame, state
                            );
                        }
                    }
                    _ => {
                        error!(
                            "invalid frame in phase = {:?},frame = {:?}",
                            self.phase, frame
                        );
                    }
                }
            }
            Frame::Voice { data } => {
                let state = self.listener.get_state();
                match &self.current_round {
                    Some(_round) => {
                        self.listener.listen(data).await;
                        if state == crate::ws::session::listener::ListenState::End {
                            self.handle_listen_end().await;
                            let silence_voice_timeout = self
                                .config
                                .silence_voice_timeout
                                .expect("logic silence voice timeout is empty");
                            self.listener.reset(Some(silence_voice_timeout)).await;
                            self.update_latest_activity_time().await;
                        }
                    }
                    None => {
                        if state == crate::ws::session::listener::ListenState::End {
                            self.handle_listen_end().await;
                            let silence_voice_timeout = self
                                .config
                                .silence_voice_timeout
                                .expect("logic silence voice timeout is empty");
                            self.listener.reset(Some(silence_voice_timeout)).await;
                            self.update_latest_activity_time().await;
                        } else {
                            self.listener.listen(data).await;
                        }
                    }
                }
                let is_speech = match self.listener.get_state() {
                    listener::ListenState::Listening(speech) => speech,
                    _ => false,
                };
                if is_speech {
                    self.update_latest_activity_time().await;
                } else {
                    let latest_activity_time = self.get_latest_activity_time().await;
                    if let (Some(latest_activity_time), Some(close_connection_no_voice_time)) = (
                        latest_activity_time,
                        self.config.close_connection_no_voice_time,
                    ) {
                        let offset_time = Local::now().timestamp_millis() - latest_activity_time;
                        if offset_time >= close_connection_no_voice_time {
                            info!(
                                target:"session",
                                "session stop: offset_time = {} >= close_connection_no_voice_time = {}",
                                offset_time, close_connection_no_voice_time
                            );
                            self.stop().await;
                        }
                    }
                }
            }
            _ => {
                error!(
                    "invalid frame in phase = {:?},frame = {:?}",
                    self.phase, frame
                );
            }
        }
    }

    async fn handle_phase_listen_for_manual_mode<'a>(&mut self, frame: &Frame<'a>) {
        match frame {
            Frame::Listen(listen_message) => {
                let state = &listen_message.state;
                match state {
                    ListenState::Start => {
                        let mode = &listen_message.mmod;
                        if let Some(mode) = mode {
                            match mode {
                                service::chobits::message::listen::ListenMode::Auto => {
                                    self.phase = Phase::Listen(ListenMode::Auto);
                                    self.current_mode = RoundMode::Auto;
                                    let silence_voice_timeout = self
                                        .config
                                        .silence_voice_timeout
                                        .expect("logic silence voice timeout is empty");
                                    //reset listener to option(slinent condition limit)
                                    self.listener.reset(Some(silence_voice_timeout)).await;
                                }
                                service::chobits::message::listen::ListenMode::Manual => {
                                    self.phase = Phase::Listen(ListenMode::Manual);
                                    self.current_mode = RoundMode::Manual;
                                    self.listener.reset(None).await;
                                }
                                service::chobits::message::listen::ListenMode::RealTime => {
                                    self.phase = Phase::Listen(ListenMode::RealTime);
                                    self.current_mode = RoundMode::RealTime;
                                }
                            }
                        } else {
                            error!(
                                "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                                self.phase, frame, state
                            );
                        }
                    }
                    ListenState::Stop => {
                        self.listener
                            .set_state(crate::ws::session::listener::ListenState::End);
                        self.handle_listen_end().await;
                    }
                    ListenState::Detect => {
                        let text = &listen_message.text;
                        match text {
                            Some(text) => {
                                self.new_round(RoundMode::Text).await;
                                //if match walk word
                                if let Some(round) = &mut self.current_round {
                                    // handle send text
                                    round.accept_command(Command::Chat { text }).await;
                                } else {
                                    panic!("current round is none");
                                }
                            }
                            None => {
                                error!(
                                    "invalid frame in phase = {:?},frame = {:?}",
                                    self.phase, frame
                                );
                            }
                        }
                    }
                    _ => {
                        error!(
                            "invalid frame in phase = {:?},frame = {:?}",
                            self.phase, frame
                        );
                    }
                }
            }
            Frame::Voice { data } => {
                self.listener.listen(data).await;
            }
            _ => {
                error!(
                    "invalid frame in phase = {:?},frame = {:?}",
                    self.phase, frame
                );
            }
        }
    }

    async fn handle_phase_listen_for_realtime_mode<'a>(&mut self, frame: &Frame<'a>) {
        match frame {
            Frame::Listen(listen_message) => {
                let state = &listen_message.state;
                match state {
                    ListenState::Start => {
                        let mode = &listen_message.mmod;
                        if let Some(mode) = mode {
                            match mode {
                                service::chobits::message::listen::ListenMode::Auto => {
                                    self.phase = Phase::Listen(ListenMode::Auto);
                                    self.current_mode = RoundMode::Auto;
                                }
                                service::chobits::message::listen::ListenMode::Manual => {
                                    self.phase = Phase::Listen(ListenMode::Manual);
                                    self.current_mode = RoundMode::Manual;
                                    self.listener.reset(None).await;
                                }
                                service::chobits::message::listen::ListenMode::RealTime => {
                                    self.phase = Phase::Listen(ListenMode::RealTime);
                                    self.current_mode = RoundMode::RealTime;
                                }
                            }
                        } else {
                            error!(
                                "invalid frame in phase = {:?},frame = {:?}, state = {:?}",
                                self.phase, frame, state
                            );
                        }
                    }
                    ListenState::Detect => {
                        let text = &listen_message.text;
                        match text {
                            Some(text) => {
                                self.update_latest_activity_time().await;
                                self.new_round(self.current_mode).await;
                                //if match walk word
                                if let Some(round) = &mut self.current_round {
                                    // TODO: detech voice id
                                    self.listener
                                        .set_state(crate::ws::session::listener::ListenState::End);
                                    let command = self.listener.get_result().await;
                                    match command {
                                        Ok(_command) => {
                                            // TODO: handle command
                                            //say hello
                                            round.accept_command(Command::Wake { text }).await;
                                        }
                                        Err(e) => {
                                            error!("{:?}", e);
                                        }
                                    }
                                    let silence_voice_timeout = self
                                        .config
                                        .silence_voice_timeout
                                        .expect("logic silence voice timeout is empty");
                                    //reset listener to option(slinent condition limit)
                                    self.listener.reset(Some(silence_voice_timeout)).await;
                                } else {
                                    panic!("current round is none");
                                }
                            }
                            None => {
                                error!(
                                    "invalid frame in phase = {:?},frame = {:?}",
                                    self.phase, frame
                                );
                            }
                        }
                    }
                    _ => {
                        error!(
                            "invalid frame in phase = {:?},frame = {:?}",
                            self.phase, frame
                        );
                    }
                }
            }
            Frame::Voice { data } => {
                let state = self.listener.get_state();
                match &self.current_round {
                    Some(_round) => {
                        self.listener.listen(data).await;
                        if state == crate::ws::session::listener::ListenState::End {
                            self.handle_listen_end().await;
                            let silence_voice_timeout = self
                                .config
                                .silence_voice_timeout
                                .expect("logic silence voice timeout is empty");
                            self.listener.reset(Some(silence_voice_timeout)).await;
                            self.update_latest_activity_time().await;
                        }
                    }
                    None => {
                        if state == crate::ws::session::listener::ListenState::End {
                            self.handle_listen_end().await;
                            let silence_voice_timeout = self
                                .config
                                .silence_voice_timeout
                                .expect("logic silence voice timeout is empty");
                            self.listener.reset(Some(silence_voice_timeout)).await;
                            self.update_latest_activity_time().await;
                        } else {
                            self.listener.listen(data).await;
                        }
                    }
                }
                let is_speech = match self.listener.get_state() {
                    listener::ListenState::Listening(speech) => speech,
                    _ => false,
                };
                if is_speech {
                    self.update_latest_activity_time().await;
                } else {
                    let latest_activity_time = self.get_latest_activity_time().await;
                    if let (Some(latest_activity_time), Some(close_connection_no_voice_time)) = (
                        latest_activity_time,
                        self.config.close_connection_no_voice_time,
                    ) {
                        let offset_time = Local::now().timestamp_millis() - latest_activity_time;
                        if offset_time >= close_connection_no_voice_time {
                            info!(
                                target:"session",
                                "session stop: offset_time = {} >= close_connection_no_voice_time = {}",
                                offset_time, close_connection_no_voice_time
                            );
                            self.stop().await;
                        }
                    }
                }
            }
            _ => {
                error!(
                    "invalid frame in phase = {:?},frame = {:?}",
                    self.phase, frame
                );
            }
        }
    }

    async fn handle_connect(&mut self, _hello_message: &HelloMessage) {
        let tx = self.output_tx.clone().expect("output tx not exists");
        let audio_config = &self.audio_config;
        let data = HelloMessage {
            message: service::chobits::message::Message {
                mtype: service::chobits::message::Type::Hello,
            },
            transport: Some(Transport::Websocket),
            audio_params: Some(AudioParam {
                format: AudioFormat::Opus,
                sample_rate: audio_config
                    .output_sample_rate
                    .expect("output sample rate is empty"),
                channels: audio_config
                    .output_channel
                    .expect("output channel is empty"),
                frame_duration: audio_config
                    .output_frame_duration
                    .expect("output frame duration is empty"),
            }),
            version: None,
            features: None,
            session_id: Some(self.id.clone()),
        };
        let result = tx.send(Ok(FrameResult::HelloResult(data))).await;
        if result.is_err() {
            info!(target:"session","tx send hello result failure");
        }
    }

    async fn handle_listen_end(&mut self) {
        let voice_pcm = self.listener.get_voice_data().await;
        let sample_rate = self
            .audio_config
            .input_sample_rate
            .expect("input sample rate is empty");

        self.new_round(self.current_mode).await;

        let command = self.listener.get_result().await;
        match command {
            Ok(command) => {
                info!(                                target:"session",
"command = {:?}", command.clone());
                let round_id = self
                    .current_round
                    .as_ref()
                    .map(|r| r.id.clone())
                    .unwrap_or_default();
                if !voice_pcm.is_empty() {
                    for observer in &self.observers {
                        observer.on_asr(&AsrContext {
                            round_id: round_id.clone(),
                            voice_pcm: voice_pcm.clone(),
                            sample_rate,
                            text: command.text.clone(),
                            confidence: command.prob,
                        });
                    }
                }
                let text = command.text.as_str();
                let is_speech_clear = self.is_speech_clear(&command.text, command.prob);
                if let Some(round) = &mut self.current_round {
                    if is_speech_clear {
                        round.accept_command(Command::AsrChat { text }).await;
                    } else {
                        round.accept_command(Command::ListenUnclear { text }).await;
                    }
                } else {
                    panic!("current round is none");
                }
            }
            Err(e) => {
                error!("{:?}", e);
                self.stop_round().await;
            }
        }
    }

    pub fn is_speech_clear(&self, text: &str, prob: f32) -> bool {
        !text.is_empty() && prob >= 0.8
    }
}
