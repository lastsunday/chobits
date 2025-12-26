#[cfg(test)]
mod tests {
    use api::llm::chat::Chat;
    use tracing::info;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    /// cargo test --test chat_test -- tests::test_chat_sentence --nocapture
    async fn test_chat_sentence() {
        let mut chat = Chat::new();
        let sentences = chat.accept_text(r#"Hello,World!My name is"#);
        assert_eq!(1, sentences.len());
        let sentences = chat.accept_text(r#"lastsunday。I like rust。I want a chobits "#);
        assert_eq!(2, sentences.len());
        let sentences = chat.accept_final();
        assert_eq!(1, sentences.len());
    }

    #[tokio::test]
    #[traced_test]
    /// cargo test --test chat_test -- tests::test_chat_sentence_new_line --nocapture
    async fn test_chat_sentence_new_line() {
        let mut chat = Chat::new();
        let sentences = chat.accept_text(
            r#"1
        "#,
        );
        assert_eq!(0, sentences.len());
        let sentences = chat.accept_text(r#"+1"#);
        assert_eq!(0, sentences.len());
        let sentences = chat.accept_text(r#"=2"#);
        assert_eq!(0, sentences.len());
        let sentences = chat.accept_final();
        for msg in sentences.iter() {
            info!("{}", msg);
        }
    }
}
