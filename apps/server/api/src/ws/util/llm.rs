use fancy_regex::Regex;
use std::{collections::HashMap, sync::LazyLock};

use std::sync::OnceLock;

fn regex() -> &'static Vec<Regex> {
    static REGEX: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEX.get_or_init(|| {
        vec![
            Regex::new(r"\n").unwrap(),
            Regex::new(r"\*\*").unwrap(),
            Regex::new(r"```.*?```").unwrap(),
            Regex::new(r"^#+\s*").unwrap(),
            Regex::new(r"(\*\*|__)(.*?)\1").unwrap(),
            Regex::new(r"(\*|_)(?=\S)(.*?)(?<=\S)\1").unwrap(),
            Regex::new(r"!\[.*?\]\(.*?\)").unwrap(),
            Regex::new(r"\[(.*?)\]\(.*?\)").unwrap(),
            Regex::new(r"^\s*>+\s*").unwrap(),
            Regex::new(r"^\s*[*+-]\s*").unwrap(),
            Regex::new(r"\$\$.*?\$\$").unwrap(),
        ]
    })
}

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

//TODO: text not use
pub fn analyze_emotion(_text: &str) -> &str {
    // TODO: use llm to analyze emotion
    r##"happy"##
}

pub fn filter_think(text: &str) -> Option<String> {
    let regex = regex::Regex::new(r"(<think>[\s\S]*</think>[\s]*)([\s\S]*)").unwrap();
    let (_full, [_think, content]) = regex.captures(text).map(|caps| caps.extract())?;
    Some(content.to_string())
}

pub fn filter(text: &str) -> Option<String> {
    let mut content = String::from(text);
    let regex = regex().iter();
    for r in regex {
        content = String::from(r.replace_all(&content, ""));
    }
    Some(content.trim().to_string())
}
