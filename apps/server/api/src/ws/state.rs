use chrono::Local;

#[derive(Debug, Default, Clone)]
pub struct State {
    pub client_speaking: bool,
    pub last_activity_time: Option<i64>,
}

impl State {
    pub fn new() -> Self {
        Self {
            client_speaking: false,
            last_activity_time: None,
        }
    }

    pub fn update_last_activity_time(&mut self) {
        self.last_activity_time = Some(Local::now().timestamp_millis());
    }
}
