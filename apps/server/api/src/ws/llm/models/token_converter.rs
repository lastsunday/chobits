use rig::streaming::RawStreamingChoice;

const THINK_START_TAG: &str = r#"<think>"#;
const THINK_END_TAG: &str = r#"</think>"#;

#[derive(Default)]
pub struct TokenConverter {
    phase: Phase,
    text_collector: String,
}

impl TokenConverter {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            text_collector: String::new(),
        }
    }

    pub fn accept_text(
        &mut self,
        text: &str,
    ) -> Vec<RawStreamingChoice<rig::providers::openai::streaming::StreamingCompletionResponse>>
    {
        let mut result = Vec::new();
        self.text_collector.push_str(text);

        match self.phase {
            Phase::Idle => {
                let has_think_tag_start = self.text_collector.contains(THINK_START_TAG);
                let has_think_tag_end = self.text_collector.contains(THINK_END_TAG);
                if has_think_tag_start && has_think_tag_end {
                    self.phase = Phase::Text;
                    let regex =
                        regex::Regex::new(r"(<think>[\s\S]*</think>[\s]*)([\s\S]*)").unwrap();
                    let (_full, [think, content]) = regex
                        .captures(&self.text_collector)
                        .map(|caps| caps.extract())
                        .unwrap();
                    result.push(RawStreamingChoice::Reasoning {
                        id: None,
                        reasoning: think.to_string(),
                    });
                    result.push(RawStreamingChoice::Message(content.to_string()));
                    self.text_collector.clear();
                } else if has_think_tag_start {
                    self.phase = Phase::Thinking;
                    let regex = regex::Regex::new(r"(<think>[\s\S]*)").unwrap();
                    let (_full, [think]) = regex
                        .captures(&self.text_collector)
                        .map(|caps| caps.extract())
                        .unwrap();
                    result.push(RawStreamingChoice::Reasoning {
                        id: None,
                        reasoning: think.to_string(),
                    });
                    self.text_collector.clear();
                } else {
                    //skip
                }
            }
            Phase::Thinking => {
                let has_think_tag_end = self.text_collector.contains(THINK_END_TAG);
                if has_think_tag_end {
                    self.phase = Phase::Text;
                    let regex = regex::Regex::new(r"([\s\S]*</think>[\s]*)([\s\S]*)").unwrap();
                    let (_full, [think, content]) = regex
                        .captures(&self.text_collector)
                        .map(|caps| caps.extract())
                        .unwrap();
                    result.push(RawStreamingChoice::Reasoning {
                        id: None,
                        reasoning: think.to_string(),
                    });
                    result.push(RawStreamingChoice::Message(content.to_string()));
                    self.text_collector.clear();
                } else {
                    result.push(RawStreamingChoice::Reasoning {
                        id: None,
                        reasoning: self.text_collector.to_string(),
                    });
                    self.text_collector.clear();
                }
            }
            Phase::Text => {
                result.push(RawStreamingChoice::Message(self.text_collector.to_string()));
                self.text_collector.clear();
            }
        }
        result
    }

    pub fn accept_final_text(
        &mut self,
        text: &str,
    ) -> Vec<RawStreamingChoice<rig::providers::openai::streaming::StreamingCompletionResponse>>
    {
        let mut result = Vec::new();
        self.text_collector.push_str(text);
        result.push(RawStreamingChoice::Message(self.text_collector.to_string()));
        self.text_collector.clear();
        result
    }
}

#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    Thinking,
    Text,
}
