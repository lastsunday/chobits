#[cfg(test)]
mod tests {
    use api::ws::llm::{Llm, LlmQwen, llm_cache::LlmCache};
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_llm() {
        LlmCache::init().await;
        let llm = LlmCache::global().instance.clone();
        let llm = llm.lock().await;
        let mut output = llm.chat(String::from("Who are you"));
        while let Some(text) = output.next().await {
            print!("{text}");
        }
    }
}
