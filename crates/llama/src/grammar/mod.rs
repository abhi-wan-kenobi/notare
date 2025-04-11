use include_url_macro::include_url;

pub const MARKDOWN_GRAMMAR: &str = include_str!("./markdown.gbnf");

#[allow(dead_code)]
pub const JSON_ARR_GRAMMAR: &str = include_url!(
    "https://raw.githubusercontent.com/ggml-org/llama.cpp/7a84777/grammars/json_arr.gbnf"
);

#[allow(dead_code)]
pub const JSON_GRAMMAR: &str =
    include_url!("https://raw.githubusercontent.com/ggml-org/llama.cpp/7a84777/grammars/json.gbnf");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_grammar() {
        let test_cases = vec![
            (
                "root ::= .*",
                include_url!("https://raw.githubusercontent.com/codecrafters-io/build-your-own-x/refs/heads/master/README.md"),
                true
            ),
            (
                MARKDOWN_GRAMMAR,
                indoc::indoc! {r#"
                Here's a response:

                # This is a test"#},
                false,
            ),
        ];

        let gbnf = gbnf_validator::Validator::new().unwrap();

        for (i, (grammar, text, expected)) in test_cases.iter().enumerate() {
            match gbnf.validate(grammar, text) {
                Ok(validated) => assert_eq!(validated, *expected, "{}th_test_case_failed", i),
                Err(e) => {
                    panic!("{}th_test_case_failed: {}", i, e);
                }
            }
        }
    }
}
