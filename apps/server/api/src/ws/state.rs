#[derive(Debug, Default, Clone)]
pub struct State {
    pub client_speaking: bool,
}

impl State {
    pub fn new() -> Self {
        Self {
            client_speaking: false,
        }
    }
}
