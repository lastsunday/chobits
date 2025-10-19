#[cfg(test)]
mod tests {
    use api::ws::llm::models::token_converter::TokenConverter;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    /// cargo test --test token_converter_test -- tests::test_token_convert_think_in_one --show-output
    async fn test_token_convert_think_in_one() {
        let mut token_converter = TokenConverter::new();
        let message = token_converter.accept_text(
            r#"<think>

</think>

1
"#,
        );
        assert_eq!(2, message.len());
        let message = token_converter.accept_text(r#"+1"#);
        assert_eq!(1, message.len());
        let message = token_converter.accept_final_text(r#"=2"#);
        assert_eq!(1, message.len());
    }

    #[tokio::test]
    #[traced_test]
    /// cargo test --test token_converter_test -- tests::test_token_convert_think_start --show-output
    async fn test_token_convert_think_start() {
        let mut token_converter = TokenConverter::new();
        let message = token_converter.accept_text(
            r#"<think>

"#,
        );
        assert_eq!(1, message.len());
        let message = token_converter.accept_text(
            r#"
</think>

1
"#,
        );
        assert_eq!(2, message.len());
        let message = token_converter.accept_text(r#"+1"#);
        assert_eq!(1, message.len());
        let message = token_converter.accept_final_text(r#"=2"#);
        assert_eq!(1, message.len());
    }
}
