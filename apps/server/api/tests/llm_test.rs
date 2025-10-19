#[cfg(test)]
mod tests {
    use api::ws::llm::LlmFactory;
    use rig::{
        OneOrMany,
        completion::CompletionRequest,
        message::{Message, Text, UserContent},
    };
    use tokio_stream::StreamExt;
    use tracing::{error, info};
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// cargo test --test llm_test --features cuda -- tests::test_llm_chat --ignored --show-output
    async fn test_llm_chat() {
        LlmFactory::init().await;
        let llm = LlmFactory::global().get_client();
        let system_prompt = "你是一个助手，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等并且数字使用中文字代替。".to_string();
        let text = "静夜思的内容".to_string();
        let chat_history = OneOrMany::<Message>::one(Message::User {
            content: OneOrMany::<UserContent>::one(UserContent::Text(Text { text: text.clone() })),
        });
        let request = CompletionRequest {
            preamble: Some(system_prompt),
            chat_history,
            documents: vec![],
            tools: vec![],
            temperature: Some(0.8),
            max_tokens: Some(999),
            tool_choice: None,
            additional_params: None,
        };
        let mut output = llm.chat(request);
        let mut result = Vec::new();
        while let Some(text) = output.next().await {
            match text {
                Ok(text) => {
                    result.push(text);
                }
                Err(e) => {
                    error!("{}", e.to_string());
                }
            }
        }
        let result: String = result.into_iter().collect();
        assert_ne!(0, result.len());
        info!("{}", result);
    }

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// cargo test --test llm_test --features cuda -- tests::test_llm_short_question --ignored --show-output
    async fn test_llm_short_question() {
        LlmFactory::init().await;
        let llm = LlmFactory::global().get_client();
        let system_prompt = "你是一个助手。".to_string();
        let text = "1+1=".to_string();
        let chat_history = OneOrMany::<Message>::one(Message::User {
            content: OneOrMany::<UserContent>::one(UserContent::Text(Text { text: text.clone() })),
        });
        let request = CompletionRequest {
            preamble: Some(system_prompt),
            chat_history,
            documents: vec![],
            tools: vec![],
            temperature: Some(0.8),
            max_tokens: Some(999),
            tool_choice: None,
            additional_params: None,
        };
        let mut output = llm.chat(request);
        let mut result = Vec::new();
        while let Some(text) = output.next().await {
            match text {
                Ok(text) => {
                    result.push(text);
                }
                Err(e) => {
                    error!("{}", e.to_string());
                }
            }
        }
        let result: String = result.into_iter().collect();
        info!("{}", result);
        assert_ne!(0, result.len());
    }
}
