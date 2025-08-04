#[cfg(test)]
mod tests {
    use api::ws::llm::{Llm, llm_cache::LlmCache};
    use tokio_stream::StreamExt;

    #[tokio::test]
    #[ignore]
    async fn test_llm_chat() {
        LlmCache::init().await;
        let llm = LlmCache::global().instance.clone();
        let mut output = llm.chat("你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等并且数字使用中文字代替。".to_string()
            ,String::from("静夜思的内容"));
        while let Some(text) = output.next().await {
            match text {
                Ok(text) => {
                    print!("{text}");
                }
                Err(e) => {
                    println!("{}", e.to_string());
                }
            }
        }
    }
}
