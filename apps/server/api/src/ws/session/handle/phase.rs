impl<L> Session<L>
where
    L: Listener + Send,
{
    async fn handle_phase_hello<'a>(&mut self, frame: &Frame<'a>) {
        match frame {
            Frame::Hello(hello_message) => {
                let mut has_mcp = false;
                if let Some(features) = &hello_message.features
                    && let Some(mcp) = features.mcp
                {
                    has_mcp = mcp;
                }
                if has_mcp {
                    // TODO: init MCP host
                    self.mcp_host =
                        Arc::new(Mutex::new(Some(UnionMcpHost::new(Some(self.id.clone())))));
                    // TODO: init Server MCP client
                    // TODO: init Remote Server MCP client
                }
                self.handle_connect(hello_message).await;
                self.phase = Phase::ListenDetect;
                if has_mcp {
                    let mcp_host = self.mcp_host.clone();
                    let mut mcp_host = mcp_host.lock().await;
                    let mcp_host = mcp_host.as_mut().expect("mcp host is none");
                    //init Device MCP client
                    self.request_mcp_initialize(mcp_host, hello_message).await;
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
                                }
                                service::chobits::message::listen::ListenMode::Manual => {
                                    self.phase = Phase::Listen(ListenMode::Manual);
                                }
                                service::chobits::message::listen::ListenMode::RealTime => {
                                    self.phase = Phase::Listen(ListenMode::RealTime);
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
                        // eps32-c3 default listen mode is none
                        // set listen mode to auto
                        self.phase = Phase::Listen(ListenMode::Auto);
                        Box::pin(self.accept_frame(frame)).await;
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
                                }
                                service::chobits::message::listen::ListenMode::Manual => {
                                    self.phase = Phase::Listen(ListenMode::Manual);
                                    self.listener.reset(None).await;
                                }
                                service::chobits::message::listen::ListenMode::RealTime => {
                                    self.phase = Phase::Listen(ListenMode::RealTime);
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
                                info!("detect text = {}", text.to_string());
                                self.update_latest_activity_time().await;
                                self.new_round().await;
                                //if match walk word
                                if let Some(round) = &mut self.current_round {
                                    // TODO: detech voice id
                                    self.listener
                                        .set_state(crate::ws::session::listener::ListenState::End);
                                    let command = self.listener.get_result().await;
                                    match command {
                                        Ok(command) => {
                                            info!("command  = {:?}", command);
                                            let mode = &listen_message.mmod;
                                            let mut is_text_message = false;
                                            if let Some(mode) = mode {
                                                is_text_message = mode.clone() == service::chobits::message::listen::ListenMode::Manual;
                                            }
                                            if is_text_message {
                                                // text message handle
                                                round.accept_command(Command::Chat { text }).await;
                                            } else {
                                                // TODO: replace text to command.text
                                                //say hello
                                                round.accept_command(Command::Wake { text }).await;
                                            }
                                        }
                                        Err(e) => {
                                            error!("{:?}", e);
                                        }
                                    }
                                    let silence_voice_timeout =
                                        config::get().logic().silence_voice_timeout();
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
                let mut round_end = true;
                match &self.current_round {
                    Some(round) => {
                        round_end = round.end.load(Ordering::Relaxed);
                        // info!(
                        //     "listener listen round end = {} state = {:?}",
                        //     round_end, state,
                        // );
                        if round_end {
                            //round is end
                            if state == crate::ws::session::listener::ListenState::End {
                                self.handle_listen_end().await;
                                let silence_voice_timeout =
                                    config::get().logic().silence_voice_timeout();
                                self.listener.reset(Some(silence_voice_timeout)).await;
                                self.update_latest_activity_time().await;
                            } else {
                                self.listener.listen(data).await;
                            }
                        } else {
                            //round is running
                        }
                    }
                    None => {
                        if state == crate::ws::session::listener::ListenState::End {
                            self.handle_listen_end().await;
                            let silence_voice_timeout =
                                config::get().logic().silence_voice_timeout();
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
                if !round_end || is_speech {
                    self.update_latest_activity_time().await;
                } else {
                    let latest_activity_time = self.get_latest_activity_time().await;
                    if let (Some(latest_activity_time), Some(close_connection_no_voice_time)) =
                        (latest_activity_time, self.close_connection_no_voice_time)
                    {
                        let offset_time = Local::now().timestamp_millis() - latest_activity_time;
                        if offset_time >= close_connection_no_voice_time {
                            self.stop().await;
                        }
                    }
                }
                // info!("latest_activity_time = {:?}", self.latest_activity_time);
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
                                    let silence_voice_timeout =
                                        config::get().logic().silence_voice_timeout();
                                    //reset listener to option(slinent condition limit)
                                    self.listener.reset(Some(silence_voice_timeout)).await;
                                }
                                service::chobits::message::listen::ListenMode::Manual => {
                                    self.phase = Phase::Listen(ListenMode::Manual);
                                    self.listener.reset(None).await;
                                    self.new_round().await;
                                }
                                service::chobits::message::listen::ListenMode::RealTime => {
                                    self.phase = Phase::Listen(ListenMode::RealTime);
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
                                info!("detect text = {}", text.to_string());
                                self.new_round().await;
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
                                }
                                service::chobits::message::listen::ListenMode::Manual => {
                                    self.phase = Phase::Listen(ListenMode::Manual);
                                    self.listener.reset(None).await;
                                }
                                service::chobits::message::listen::ListenMode::RealTime => {
                                    self.phase = Phase::Listen(ListenMode::RealTime);
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
                                info!("detect text = {}", text.to_string());
                                self.update_latest_activity_time().await;
                                self.new_round().await;
                                //if match walk word
                                if let Some(round) = &mut self.current_round {
                                    // TODO: detech voice id
                                    self.listener
                                        .set_state(crate::ws::session::listener::ListenState::End);
                                    let command = self.listener.get_result().await;
                                    match command {
                                        Ok(command) => {
                                            info!("command  = {:?}", command);
                                            //say hello
                                            round.accept_command(Command::Wake { text }).await;
                                        }
                                        Err(e) => {
                                            error!("{:?}", e);
                                        }
                                    }
                                    let silence_voice_timeout =
                                        config::get().logic().silence_voice_timeout();
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
                        // info!(
                        //     "listener listen round end = {} state = {:?}",
                        //     round_end, state,
                        // );
                        self.listener.listen(data).await;
                        if state == crate::ws::session::listener::ListenState::End {
                            self.handle_listen_end().await;
                            let silence_voice_timeout =
                                config::get().logic().silence_voice_timeout();
                            self.listener.reset(Some(silence_voice_timeout)).await;
                            self.update_latest_activity_time().await;
                        }
                    }
                    None => {
                        if state == crate::ws::session::listener::ListenState::End {
                            self.handle_listen_end().await;
                            let silence_voice_timeout =
                                config::get().logic().silence_voice_timeout();
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
                    if let (Some(latest_activity_time), Some(close_connection_no_voice_time)) =
                        (latest_activity_time, self.close_connection_no_voice_time)
                    {
                        //connection timeout handle
                        let offset_time = Local::now().timestamp_millis() - latest_activity_time;
                        // info!("offset_time = {}", offset_time);
                        if offset_time >= close_connection_no_voice_time {
                            self.stop().await;
                        }
                    }
                }
                // info!("latest_activity_time = {:?}", self.latest_activity_time);
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
        let tx = self.output_tx.clone().unwrap();
        let audio_config = config::get().audio();
        let data = HelloMessage {
            message: service::chobits::message::Message {
                mtype: service::chobits::message::Type::Hello,
            },
            transport: Some(Transport::Websocket),
            audio_params: Some(AudioParam {
                format: AudioFormat::Opus,
                sample_rate: audio_config.output_sample_rate(),
                channels: audio_config.output_channel(),
                frame_duration: audio_config.output_frame_duration(),
            }),
            version: None,
            features: None,
            session_id: Some(self.id.clone()),
        };
        let result = tx.send(Ok(FrameResult::HelloResult(data))).await;
        if result.is_err() {
            info!("tx send hello result failure");
        }
    }

    async fn handle_listen_end(&mut self) {
        let command = self.listener.get_result().await;
        match command {
            Ok(command) => {
                self.new_round().await;
                info!("command = {:?}", command.clone());
                let text = command.text.as_str();
                let is_speech_clear = self.is_speech_clear(command.prob);
                if let Some(round) = &mut self.current_round {
                    if is_speech_clear {
                        round.accept_command(Command::Chat { text }).await;
                    } else {
                        round.accept_command(Command::ListenUnclear { text }).await;
                    }
                } else {
                    panic!("current round is none");
                }
            }
            Err(e) => {
                error!("{:?}", e);
            }
        }
    }

    pub fn is_speech_clear(&self, prob: f32) -> bool {
        prob >= 0.8
    }
}
