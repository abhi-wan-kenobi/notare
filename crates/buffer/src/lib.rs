#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to parse markdown")]
    MarkdownParseError(String),
    #[error("failed to render markdown")]
    MarkdownRenderError(String),
    #[error("failed to render html")]
    HTMLRenderError(String),
    #[error("failed to parse html")]
    HTMLParseError(String),
}

pub fn opinionated_md_to_html(text: impl AsRef<str>) -> Result<String, Error> {
    let md = md_to_md(text)?;
    md_to_html(&md)
}

fn md_to_md(text: impl AsRef<str>) -> Result<String, Error> {
    let mut ast = markdown::to_mdast(text.as_ref(), &markdown::ParseOptions::default())
        .map_err(|e| Error::MarkdownParseError(e.to_string()))?;

    let transformations: Vec<Box<dyn Fn(&mut markdown::mdast::Node)>> = vec![
        Box::new(|node| {
            set_heading_level_from(node, 1, false);
        }),
        Box::new(flatten_headings),
        Box::new(convert_ordered_to_unordered),
    ];

    for transform in transformations {
        transform(&mut ast);
    }

    let md = mdast_util_to_markdown::to_markdown_with_options(
        &ast,
        &mdast_util_to_markdown::Options::default(),
    )
    .map_err(|e| Error::MarkdownRenderError(e.to_string()))?;

    Ok(md)
}

fn md_to_html(text: &str) -> Result<String, Error> {
    let html = markdown::to_html_with_options(
        text,
        &markdown::Options {
            parse: markdown::ParseOptions::default(),
            compile: markdown::CompileOptions {
                allow_dangerous_html: true,
                ..Default::default()
            },
        },
    )
    .map_err(|e| Error::HTMLRenderError(e.to_string()))?;

    let dom = tl::parse(&html, tl::ParserOptions::default())
        .map_err(|e| Error::HTMLParseError(e.to_string()))?;

    Ok(dom.outer_html())
}

fn convert_ordered_to_unordered(node: &mut markdown::mdast::Node) {
    if let markdown::mdast::Node::List(list) = node {
        list.ordered = false;
        list.spread = false;
    }

    if let Some(children) = node.children_mut() {
        for child in children {
            convert_ordered_to_unordered(child);
        }
    }
}

fn set_heading_level_from(node: &mut markdown::mdast::Node, depth: u8, header_found: bool) -> bool {
    let mut found_any_heading = header_found;

    if let markdown::mdast::Node::Heading(heading) = node {
        found_any_heading = true;
        heading.depth = depth;

        if let Some(children) = node.children_mut() {
            for child in children {
                set_heading_level_from(child, depth + 1, found_any_heading);
            }
        }
    } else if let Some(children) = node.children_mut() {
        for child in children {
            let child_found = set_heading_level_from(
                child,
                if found_any_heading { depth + 1 } else { depth },
                found_any_heading,
            );
            found_any_heading = found_any_heading || child_found;
        }
    }

    found_any_heading
}

fn flatten_headings(node: &mut markdown::mdast::Node) {
    if let markdown::mdast::Node::Heading(heading) = node {
        if heading.depth > 1 {
            let children = node.children().cloned().unwrap_or_default();

            let strong_node = markdown::mdast::Node::Strong(markdown::mdast::Strong {
                children,
                position: None,
            });

            *node = markdown::mdast::Node::Paragraph(markdown::mdast::Paragraph {
                children: vec![strong_node],
                position: None,
            });
        }
    }

    if let Some(children) = node.children_mut() {
        for child in children {
            flatten_headings(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1() {
        let input = r#"
# Hello

## World

1. Hi
2. Bye!
"#;

        let output_expected = r#"
# Hello

**World**

* Hi
* Bye!
"#;

        let output_actual = md_to_md(input).unwrap();
        assert_eq!(output_actual.trim(), output_expected.trim());
    }

    #[test]
    fn test_2() {
        let input = r#"
## Hello

### World

1. Hi
2. Bye!
"#;

        let output_expected = r#"
# Hello

**World**

* Hi
* Bye!
"#;

        let output_actual = md_to_md(input).unwrap();
        assert_eq!(output_actual.trim(), output_expected.trim());
    }
}
