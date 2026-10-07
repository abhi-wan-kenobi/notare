use std::path::Path;

use crate::{Error, ExportInput};

pub fn export_pdf(path: impl AsRef<Path>, input: impl Into<ExportInput>) -> Result<(), Error> {
    let input = input.into();
    let typst_content = crate::typst::build_typst_content(&input);
    let pdf_bytes = crate::typst::compile_to_pdf(&typst_content)?;
    std::fs::write(path.as_ref(), pdf_bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{ExportInput, ExportMetadata, Transcript, TranscriptItem};

    #[test]
    fn full_export_compiles_to_pdf() {
        let input = ExportInput {
            enhanced_md: "# Summary\n\n**Bold**, _emphasis_, ~~strike~~ and `code` with a \
                [link](https://example.com).\n\n- one\n  - nested\n\n1. first\n2. second\n\n\
                > quoted *text* #hash $math @ref\n"
                .to_string(),
            memo_md: Some("## Memo\n\nRaw note line\\\nwith a hard break.".to_string()),
            transcript: Some(Transcript {
                items: vec![
                    TranscriptItem {
                        speaker: Some("Alice".to_string()),
                        text: "Hello \"world\" [1]".to_string(),
                    },
                    TranscriptItem {
                        speaker: None,
                        text: "No speaker".to_string(),
                    },
                ],
            }),
            metadata: Some(ExportMetadata {
                title: "Weekly sync".to_string(),
                created_at: "2026-10-06".to_string(),
                participants: vec!["Alice".to_string(), "Bob".to_string()],
                event_title: Some("Team meeting".to_string()),
                duration: Some("30m".to_string()),
            }),
        };

        let content = crate::typst::build_typst_content(&input);
        let pdf = crate::typst::compile_to_pdf(&content).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
    }
}
