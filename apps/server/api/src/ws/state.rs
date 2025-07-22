use chrono::Local;

#[derive(Debug, Default, Clone)]
pub struct State {
    pub client_speaking: bool,
    pub last_activity_time: Option<i64>,
    pub last_speaking_time: Option<i64>,
}

impl State {
    pub fn new() -> Self {
        Self {
            client_speaking: false,
            last_activity_time: None,
            last_speaking_time: None,
        }
    }

    pub fn update_last_activity_time(&mut self) {
        self.last_activity_time = Some(Local::now().timestamp_millis());
    }

    pub fn update_last_speaking_time(&mut self) {
        self.last_speaking_time = Some(Local::now().timestamp_millis());
    }

    pub fn reset(&mut self) {
        self.client_speaking = false;
        self.last_activity_time = None;
        self.last_speaking_time = None;
    }
}
