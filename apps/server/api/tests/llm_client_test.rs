#[cfg(test)]
mod tests {
    use api::ws::llm::LlmFactory;
    use rig::{
        OneOrMany,
        completion::{CompletionRequest, ToolDefinition},
        message::{AssistantContent, Message, Text, UserContent},
    };
    use tokio_stream::StreamExt;
    use tracing::{error, info};
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// cargo test --test llm_client_test --features cuda -- tests::test_chat_simple --ignored --show-output
    async fn test_chat_simple() {
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
    /// cargo test --test llm_client_test --features cuda -- tests::test_short_question --ignored --show-output
    async fn test_short_question() {
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

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// cargo test --test llm_client_test --features cuda -- tests::test_chat_history --ignored --show-output
    async fn test_chat_history() {
        LlmFactory::init().await;
        let llm = LlmFactory::global().get_client();
        let system_prompt = "你是一个助手，协助用户进行记录，查询和提供建议，所有回答必须使用纯文本自然语言，禁止使用任何Markdown符号如#、-、*等并且数字使用中文字代替。".to_string();
        let mut chat_history = OneOrMany::<Message>::one(Message::User {
            content: OneOrMany::<UserContent>::one(UserContent::Text(Text {
                text: r#"记录一下，小小的电话号码为12349876"#.to_string(),
            })),
        });
        chat_history.push(Message::Assistant {
            id: None,
            content: OneOrMany::<AssistantContent>::one(AssistantContent::Text(Text {
                text: r#"小小电话号码为12349876"#.to_string(),
            })),
        });
        chat_history.push(Message::User {
            content: OneOrMany::<UserContent>::one(UserContent::Text(Text {
                text: r#"告诉我小小的电话号码"#.to_string(),
            })),
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
        assert_eq!("小小的电话号码为12349876", result);
        info!("{}", result);
    }

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// cargo test --test llm_client_test --features cuda -- tests::test_chat_mcp --ignored --show-output
    async fn test_chat_mcp() {
        LlmFactory::init().await;
        let llm = LlmFactory::global().get_client();
        let system_prompt = "".to_string();
        let tools: Vec<ToolDefinition> = vec![ToolDefinition {
            name: "get_current_weather".to_string(),
            description: "Get the current weather in a given location".to_string(),
            parameters: serde_json::from_str(
                r#"
                {
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description":"The city and state, e.g. San Francisco, CA"
                        },
                        "unit": {
                            "type": "string",
                            "enum": ["celsius", "fahrenheit"]
                        }
                    },
                    "required": ["location"]
                }
                "#,
            )
            .unwrap(),
        }];
        let chat_history = OneOrMany::<Message>::one(Message::User {
            content: OneOrMany::<UserContent>::one(UserContent::Text(Text {
                text: r#"What's the weather like in San Francisco?"#.to_string(),
            })),
        });
        let request = CompletionRequest {
            preamble: Some(system_prompt),
            chat_history,
            documents: vec![],
            tools,
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
        assert_eq!(0, result.len());
    }
}
