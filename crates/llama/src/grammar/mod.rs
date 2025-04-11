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
                "freeCodeCamp",
                "root ::= .*",
                include_url!("https://raw.githubusercontent.com/freeCodeCamp/freeCodeCamp/refs/heads/main/README.md"),
            ),
            (
                "build-your-own-x",
                "root ::= .*",
                include_url!("https://raw.githubusercontent.com/codecrafters-io/build-your-own-x/refs/heads/master/README.md"),
            ),
        ];

        for (name, grammar, content) in test_cases {
            let transformed_md = hypr_buffer::opinionated_md_to_md(content).unwrap();
            let validated = gbnf_validator::llama_gbnf_validator(grammar, &transformed_md).unwrap();
            assert!(validated, "Validation failed for: {}", name);
        }
    }
}
