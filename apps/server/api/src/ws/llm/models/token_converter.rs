use rig::streaming::RawStreamingChoice;
use serde::{Deserialize, Serialize};

const THINK_TAG_NAME: &str = r#"think"#;
const TOOL_CALL_TAG_NAME: &str = r#"tool_call"#;
const MAX_TAG_NAME_LEN: usize = 9;

#[derive(Default)]
pub struct TokenConverter {
    phase: Phase,
    text_collector: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCall {
    name: String,
    arguments: serde_json::Value,
}

impl TokenConverter {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            text_collector: String::new(),
        }
    }

    fn skip_start_tag<'a>(text: &'a str, tag_name: &'a str) -> &'a str {
        let regex = regex::Regex::new(&format!("<{}>([\\s\\S]*)", tag_name)).unwrap();
        // TODO: need handle unwrap
        let (_full, [other]) = regex.captures(text).map(|caps| caps.extract()).unwrap();
        other
    }

    fn skip_end_tag_and_get_content<'a>(text: &'a str, tag_name: &'a str) -> (&'a str, &'a str) {
        // https://github.com/javascript-tutorial/en.javascript.info/blob/master/9-regular-expressions/10-regexp-greedy-and-lazy/article.md
        let regex =
            regex::Regex::new(&format!("([\\s\\S]*?)</{}>+([\\s]*[\\s\\S]*)", tag_name)).unwrap();
        // TODO: need handle unwrap
        let (_full, [tag_content, other_content]) =
            regex.captures(text).map(|caps| caps.extract()).unwrap();
        (tag_content, other_content)
    }

    fn analyse_text(
        &mut self,
    ) -> Vec<RawStreamingChoice<rig::providers::openai::streaming::StreamingCompletionResponse>>
    {
        let mut result: Vec<
            RawStreamingChoice<rig::providers::openai::streaming::StreamingCompletionResponse>,
        > = Vec::new();
        let text = self.text_collector.clone();
        match self.phase {
            Phase::Idle => {
                if self
                    .text_collector
                    .contains(&format!("<{}>", THINK_TAG_NAME))
                {
                    let other = TokenConverter::skip_start_tag(&text, THINK_TAG_NAME);
                    self.text_collector.clear();
                    self.text_collector.push_str(other);
                    self.phase = Phase::Thinking;
                } else if self
                    .text_collector
                    .contains(&format!("<{}>", TOOL_CALL_TAG_NAME))
                {
                    let other = TokenConverter::skip_start_tag(&text, TOOL_CALL_TAG_NAME);
                    self.text_collector.clear();
                    self.text_collector.push_str(other);
                    self.phase = Phase::ToolCall;
                } else {
                    self.phase = Phase::Text;
                }
            }
            Phase::Thinking => {
                if self
                    .text_collector
                    .contains(&format!("</{}>", THINK_TAG_NAME))
                {
                    let (tag_content, other_content) =
                        TokenConverter::skip_end_tag_and_get_content(&text, THINK_TAG_NAME);
                    result.push(RawStreamingChoice::Reasoning {
                        id: None,
                        reasoning: tag_content.to_string(),
                    });
                    self.text_collector.clear();
                    self.text_collector.push_str(other_content);
                    self.phase = Phase::Idle;
                } else {
                    result.push(RawStreamingChoice::Reasoning {
                        id: None,
                        reasoning: text,
                    });
                    self.text_collector.clear();
                }
            }
            Phase::ToolCall => {
                if self
                    .text_collector
                    .contains(&format!("</{}>", TOOL_CALL_TAG_NAME))
                {
                    let (tag_content, other_content) =
                        TokenConverter::skip_end_tag_and_get_content(&text, TOOL_CALL_TAG_NAME);
                    // TODO: need handle unwrap
                    let tool_call: ToolCall = serde_json::from_str(tag_content).unwrap();
                    result.push(RawStreamingChoice::ToolCall {
                        id: "".to_string(),
                        call_id: None,
                        name: tool_call.name,
                        arguments: tool_call.arguments,
                    });
                    self.text_collector.clear();
                    self.text_collector.push_str(other_content);
                    self.phase = Phase::Idle;
                } else {
                    //skip
                }
            }
            Phase::Text => {
                result.push(RawStreamingChoice::Message(self.text_collector.to_string()));
                self.text_collector.clear();
            }
        }
        let text = &self.text_collector;
        if text.contains(&format!("<{}>", THINK_TAG_NAME))
            || text.contains(&format!("</{}>", THINK_TAG_NAME))
            || text.contains(&format!("<{}>", TOOL_CALL_TAG_NAME))
            || text.contains(&format!("</{}>", TOOL_CALL_TAG_NAME))
        {
            result.append(&mut self.analyse_text());
        }
        result
    }

    pub fn accept_text(
        &mut self,
        text: &str,
    ) -> Vec<RawStreamingChoice<rig::providers::openai::streaming::StreamingCompletionResponse>>
    {
        self.text_collector.push_str(text);
        if self.text_collector.len() >= MAX_TAG_NAME_LEN + 2 {
            self.analyse_text()
        } else {
            vec![]
        }
    }

    pub fn accept_final_text(
        &mut self,
        text: &str,
    ) -> Vec<RawStreamingChoice<rig::providers::openai::streaming::StreamingCompletionResponse>>
    {
        self.text_collector.push_str(text);
        self.analyse_text()
    }
}

#[derive(Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    Thinking,
    ToolCall,
    Text,
}

#[cfg(test)]
mod tests {
    use tracing_test::traced_test;

    use super::TokenConverter;

    #[tokio::test]
    #[traced_test]
    /// cargo test --package api --lib -- ws::llm::models::token_converter::tests::test_token_convert_think_in_one --show-output
    async fn test_token_convert_think_in_one() {
        let mut token_converter = TokenConverter::new();
        let messages = token_converter.accept_text(
            r#"<think>

            </think>

            1
            "#,
        );
        // out -> <think></think>
        // 1
        assert_eq!(1, messages.len());
        // 1+1
        let messages = token_converter.accept_text(r#"+1"#);
        assert_eq!(0, messages.len());
        // out -> 1+1=2
        let messages = token_converter.accept_final_text(r#"=2"#);
        assert_eq!(1, messages.len());
        for message in messages.iter() {
            match message {
                rig::streaming::RawStreamingChoice::Message(_text) => {
                    //skip
                }
                _ => {
                    panic!("error type,it not text");
                }
            }
        }
    }

    #[tokio::test]
    #[traced_test]
    /// cargo test --package api --lib -- ws::llm::models::token_converter::tests::test_token_convert_think_start --show-output
    async fn test_token_convert_think_start() {
        let mut token_converter = TokenConverter::new();
        let messages = token_converter.accept_text(
            r#"<think>

            "#,
        );
        assert_eq!(0, messages.len());
        let messages = token_converter.accept_text(
            r#"
            </think>

                1
            "#,
        );
        // out -> <think></think>
        // 1
        assert_eq!(1, messages.len());
        // 1+1
        let messages = token_converter.accept_text(r#"+1"#);
        assert_eq!(0, messages.len());
        // out -> 1+1=2
        let messages = token_converter.accept_final_text(r#"=2"#);
        assert_eq!(1, messages.len());
        for message in messages.iter() {
            match message {
                rig::streaming::RawStreamingChoice::Message(_text) => {
                    //skip
                }
                _ => {
                    panic!("error type,it not text");
                }
            }
        }
    }

    #[tokio::test]
    #[traced_test]
    /// cargo test --package api --lib -- ws::llm::models::token_converter::tests::test_token_convert_tool_call_in_think_mode --show-output
    async fn test_token_convert_tool_call_in_think_mode() {
        let mut token_converter = TokenConverter::new();
        let mut messages = token_converter.accept_text(
            r#"<think>Okay, the user is asking for the current temperature in San Francisco and the temperature for tomorrow. Let me check the available tools.\n\nFirst, there's the get_current_temperature function. It requires the location and optionally the unit. Since the user didn't specify the unit, I'll default to celsius. The location should be \"San Francisco, State, Country\". Wait, the example format is \"City, State, Country\", but San Francisco is a city in California, USA. So the location parameter would be \"San Francisco, California, United States\".\n\nThen, for tomorrow's temperature, the user mentioned the current date is 2024-09-30, so tomorrow would be 2024-10-01. The get_temperature_date function requires location, date, and unit. Again, using the same location and default unit. I need to format the date as \"Year-Month-Day\", which is 2024-10-01.\n\nWait, the current date given is 2024-09-30. If today is September 30, then tomorrow is October 1st. So the date parameter for the second function call should be \"2024-10-01\".\n\nI should make two separate function calls: one for the current temperature and another for tomorrow's date. Let me structure the JSON for both tool calls accordingly.</think>\n
            <tool_call>{"name": "get_current_temperature", "arguments": "{\"location\": \"San Francisco, California, United States\", \"unit\": \"celsius\"}"}</tool_call>\n
            <tool_call>{"name": "get_temperature_date", "arguments": "{\"location\": \"San Francisco, California, United States\", \"date\": \"2024-10-01\", \"unit\": \"celsius\"}"}</tool_call>\n
            "#,
        );
        let message = messages.remove(0);
        if let rig::streaming::RawStreamingChoice::Reasoning { id: _id, reasoning } = message {
            assert_eq!(
                r#"Okay, the user is asking for the current temperature in San Francisco and the temperature for tomorrow. Let me check the available tools.\n\nFirst, there's the get_current_temperature function. It requires the location and optionally the unit. Since the user didn't specify the unit, I'll default to celsius. The location should be \"San Francisco, State, Country\". Wait, the example format is \"City, State, Country\", but San Francisco is a city in California, USA. So the location parameter would be \"San Francisco, California, United States\".\n\nThen, for tomorrow's temperature, the user mentioned the current date is 2024-09-30, so tomorrow would be 2024-10-01. The get_temperature_date function requires location, date, and unit. Again, using the same location and default unit. I need to format the date as \"Year-Month-Day\", which is 2024-10-01.\n\nWait, the current date given is 2024-09-30. If today is September 30, then tomorrow is October 1st. So the date parameter for the second function call should be \"2024-10-01\".\n\nI should make two separate function calls: one for the current temperature and another for tomorrow's date. Let me structure the JSON for both tool calls accordingly."#,
                reasoning
            );
        } else {
            panic!("error type,it not message");
        }
        let message = messages.remove(0);
        if let rig::streaming::RawStreamingChoice::ToolCall {
            id: _id,
            call_id: _call_id,
            name,
            arguments,
        } = message
        {
            assert_eq!("get_current_temperature", name);
            assert_eq!(
                "{\"location\": \"San Francisco, California, United States\", \"unit\": \"celsius\"}",
                arguments
            );
        } else {
            panic!("error type,it not tool call");
        }
        let message = messages.remove(0);
        if let rig::streaming::RawStreamingChoice::ToolCall {
            id: _id,
            call_id: _call_id,
            name,
            arguments,
        } = message
        {
            assert_eq!("get_temperature_date", name);
            assert_eq!(
                "{\"location\": \"San Francisco, California, United States\", \"date\": \"2024-10-01\", \"unit\": \"celsius\"}",
                arguments
            );
        } else {
            panic!("error type,it not tool call");
        }
    }

    #[tokio::test]
    #[traced_test]
    /// cargo test --package api --lib -- ws::llm::models::token_converter::tests::test_token_convert_tool_call_in_no_think_mode --show-output
    async fn test_token_convert_tool_call_in_no_think_mode() {
        let mut token_converter = TokenConverter::new();
        let mut messages = token_converter.accept_text(
            r#"<tool_call>{"name": "get_current_temperature", "arguments": "{\"location\": \"San Francisco, California, United States\", \"unit\": \"celsius\"}"}</tool_call>\n
            <tool_call>{"name": "get_temperature_date", "arguments": "{\"location\": \"San Francisco, California, United States\", \"date\": \"2024-10-01\", \"unit\": \"celsius\"}"}</tool_call>\n
            "#,
        );
        let message = messages.remove(0);
        if let rig::streaming::RawStreamingChoice::ToolCall {
            id: _id,
            call_id: _call_id,
            name,
            arguments,
        } = message
        {
            assert_eq!("get_current_temperature", name);
            assert_eq!(
                "{\"location\": \"San Francisco, California, United States\", \"unit\": \"celsius\"}",
                arguments
            );
        } else {
            panic!("error type,it not tool call");
        }
        let message = messages.remove(0);
        if let rig::streaming::RawStreamingChoice::ToolCall {
            id: _id,
            call_id: _call_id,
            name,
            arguments,
        } = message
        {
            assert_eq!("get_temperature_date", name);
            assert_eq!(
                "{\"location\": \"San Francisco, California, United States\", \"date\": \"2024-10-01\", \"unit\": \"celsius\"}",
                arguments
            );
        } else {
            panic!("error type,it not tool call");
        }
    }

    #[tokio::test]
    #[traced_test]
    /// cargo test --package api --lib -- ws::llm::models::token_converter::tests::test_token_convert_tool_call_example1 --show-output
    async fn test_token_convert_tool_call_example1() {
        let mut token_converter = TokenConverter::new();
        let text = "<tool_call>{\"name\": \"getweather\", \"arguments\": {\"location\": \"San Francisco\"}}</tool_call>";
        let mut msg_count = 0;
        for c in text.chars() {
            let mut messages = token_converter.accept_text(&c.to_string());
            if !messages.is_empty() {
                msg_count += 1;
                let message = messages.remove(0);
                if let rig::streaming::RawStreamingChoice::ToolCall {
                    id: _id,
                    call_id: _call_id,
                    name,
                    arguments,
                } = message
                {
                    assert_eq!("getweather", name);
                    assert_eq!(
                        "{\"location\":\"San Francisco\"}",
                        serde_json::to_string(&arguments).unwrap()
                    );
                } else {
                    panic!("error type,it not tool call");
                }
            }
        }
        assert_eq!(1, msg_count);
    }

    #[tokio::test]
    #[traced_test]
    /// cargo test --package api --lib -- ws::llm::models::token_converter::tests::test_token_convert_tool_call_example2 --show-output
    async fn test_token_convert_tool_call_example2() {
        let mut token_converter = TokenConverter::new();
        let text = "<tool_call>{\"name\": \"getweather\", \"arguments\": {\"location\": \"San Francisco\"}}</tool_call";
        for c in text.chars() {
            token_converter.accept_text(&c.to_string());
        }
        let messages = token_converter.accept_final_text(">");
        assert_eq!(1, messages.len());
        for message in messages.iter() {
            match message {
                rig::streaming::RawStreamingChoice::ToolCall {
                    id: _id,
                    call_id: _call_id,
                    name: _name,
                    arguments: _arguments,
                } => {
                    //skip
                }
                _ => {
                    panic!("error type,it not tool call");
                }
            }
        }
    }
}
