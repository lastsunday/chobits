#[cfg(test)]
mod tests {
    use api::ws::util::llm::filter;

    #[test]
    fn test_llm_util_filter() {
        let result = filter("<think>\n\n</think>\n\n我是一个AI助手。请问有什么可以帮助你的吗？");
        assert_eq!(result, Some("我是一个AI助手。请问有什么可以帮助你的吗？"));
    }
}
