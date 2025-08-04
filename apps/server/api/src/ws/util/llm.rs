use regex::Regex;
use std::{collections::HashMap, sync::LazyLock};

pub static EMOJI_MAP: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    let mut map: HashMap<&str, &str> = HashMap::new();
    map.insert(r#"neutral"#, r#"😶"#);
    map.insert(r#"happy"#, r#"🙂"#);
    map.insert(r#"laughing"#, r#"😆"#);
    map.insert(r#"funny"#, r#"😂"#);
    map.insert(r#"sad"#, r#"😔"#);
    map.insert(r#"angry"#, r#"😠"#);
    map.insert(r#"crying"#, r#"😭"#);
    map.insert(r#"loving"#, r#"😍"#);
    map.insert(r#"embarrassed"#, r#"😳"#);
    map.insert(r#"surprised"#, r#"😲"#);
    map.insert(r#"shocked"#, r#"😱"#);
    map.insert(r#"thinking"#, r#"🤔"#);
    map.insert(r#"winking"#, r#"😉"#);
    map.insert(r#"cool"#, r#"😎"#);
    map.insert(r#"relaxed"#, r#"😌"#);
    map.insert(r#"delicious"#, r#"🤤"#);
    map.insert(r#"kissy"#, r#"😘"#);
    map.insert(r#"confident"#, r#"😏"#);
    map.insert(r#"sleepy"#, r#"😴"#);
    map.insert(r#"silly"#, r#"😜"#);
    map.insert(r#"confused"#, r#"🙄"#);
    map
});

pub fn analyze_emotion(text: &str) -> &str {
    // TODO: use llm to analyze emotion
    return r#"happy"#;
}

pub fn filter(text: &str) -> Option<&str> {
    let regex = Regex::new(r"(<think>[\s\S]*</think>[\s]*)([\s\S]*)").unwrap();
    let Some((_full, [_think, content])) = regex.captures(text).map(|caps| caps.extract()) else {
        return None;
    };
    Some(content)
}
