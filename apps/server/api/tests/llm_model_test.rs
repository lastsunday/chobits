#[cfg(test)]
mod tests {
    use api::ws::llm::LlmFactory;
    use rig::{
        OneOrMany,
        completion::{CompletionRequest, ToolDefinition},
        message::{Message, Reasoning, Text, ToolCall, UserContent},
        streaming::StreamedAssistantContent,
    };
    use tokio_stream::StreamExt;
    use tracing::info;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    #[ignore]
    /// cargo test --test llm_model_test --features cuda -- tests::test_chat_mcp --ignored --show-output
    async fn test_chat_mcp() {
        LlmFactory::init().await;
        let model = LlmFactory::create_model();
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
        let response = model.stream(request).await;
        match response {
            Ok(mut stream) => {
                // TODO:
                while let Some(value) = stream.next().await {
                    match value {
                        Ok(StreamedAssistantContent::Text(_text)) => {
                            panic!("value type error,it not tool call");
                        }
                        Ok(StreamedAssistantContent::Final(
                            rig::providers::openai::StreamingCompletionResponse { usage },
                        )) => {
                            info!("{:?}", usage);
                        }
                        Ok(StreamedAssistantContent::ToolCall(ToolCall {
                            id: _id,
                            call_id: _call_id,
                            function,
                        })) => {
                            assert_eq!(
                                r#"{"name":"get_current_weather","arguments":{"location":"San Francisco"}}"#,
                                serde_json::to_string(&function).unwrap()
                            );
                        }
                        Ok(StreamedAssistantContent::ToolCallDelta {
                            id: _id,
                            delta: _delta,
                        }) => {
                            // TODO:
                        }
                        Ok(StreamedAssistantContent::Reasoning(Reasoning {
                            id: _id,
                            reasoning,
                            ..
                        })) => {
                            info!("{:?}", reasoning);
                        }
                        Err(e) => {
                            panic!("has completion error: {:?}", e);
                        }
                    }
                }
            }
            Err(_e) => {
                panic!("has completion error");
            }
        }
    }
}
