#[cfg(test)]
mod tests {
    use api::ws::util::llm::{filter, filter_think};

    #[tokio::test]
    /// cargo test --test llm_util_test --features cuda -- tests::test_llm_util_filter --show-output
    async fn test_llm_util_filter() {
        let result = filter_think(
            "<think>\n\n</think>\n\n我是一个AI助手。\n\n请问有什么可以帮助你的吗？\n\n",
        );
        let result = filter(&result.unwrap());
        assert_eq!(
            result,
            Some("我是一个AI助手。请问有什么可以帮助你的吗？".to_string())
        );
    }
}
