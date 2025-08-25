use tracing::info;

pub trait Listener {
    fn listen(&mut self, data: &[u8]) -> impl std::future::Future<Output = ()> + Send;
    fn get_result(&mut self) -> impl std::future::Future<Output = Option<String>> + Send;
}

#[derive(Debug)]
pub struct DefaultListener {}

impl Listener for DefaultListener {
    async fn listen(&mut self, data: &[u8]) {
        info!("listen data len = {}", data.len());
    }

    async fn get_result(&mut self) -> Option<String> {
        Some(String::from("listener dummy result"))
    }
}
