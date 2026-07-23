//! Action-item export (WS-D2): render a session's (or all) action items to
//! CSV or JSON. SQLite is authoritative; the plugin queries the rows and maps
//! them into [`ActionItemExport`], and these pure serializers turn them into a
//! file payload. Keeping the serialization here (out of the tauri plugin) makes
//! CSV escaping unit-testable without a DB or an app handle.

use serde::Serialize;

/// One exportable action item, already flattened from the `action_items` row
/// into the exact columns the CSV/JSON payloads expose.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActionItemExport {
    pub text: String,
    /// Owner label (currently the raw `owner_speaker_id`); empty when none.
    pub owner: String,
    /// ISO date (`YYYY-MM-DD`) or empty.
    pub due_at: String,
    /// `todo` | `in_progress` | `done` (mirrors SQLite).
    pub status: String,
    pub priority: String,
    pub confidence: f64,
    /// Verbatim transcript span the item was extracted from (provenance).
    pub source_text: String,
}

/// The CSV header, in column order. Kept next to the row writer so the two
/// never drift.
const CSV_HEADER: &str = "text,owner,due_at,status,priority,confidence,source_text";

/// RFC-4180 field escaping: wrap in double quotes and double any embedded quote
/// whenever the field contains a comma, quote, or newline.
fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Render items as an RFC-4180 CSV document (header + one row per item, CRLF
/// line endings). `confidence` is emitted with its default float formatting.
pub fn to_csv(items: &[ActionItemExport]) -> String {
    let mut out = String::from(CSV_HEADER);
    out.push_str("\r\n");
    for item in items {
        let confidence = item.confidence.to_string();
        let fields = [
            csv_escape(&item.text),
            csv_escape(&item.owner),
            csv_escape(&item.due_at),
            csv_escape(&item.status),
            csv_escape(&item.priority),
            csv_escape(&confidence),
            csv_escape(&item.source_text),
        ];
        out.push_str(&fields.join(","));
        out.push_str("\r\n");
    }
    out
}

/// Render items as a pretty-printed JSON array.
pub fn to_json(items: &[ActionItemExport]) -> crate::Result<String> {
    serde_json::to_string_pretty(items).map_err(|e| crate::Error::Json(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(text: &str, owner: &str) -> ActionItemExport {
        ActionItemExport {
            text: text.to_string(),
            owner: owner.to_string(),
            due_at: "2026-07-24".to_string(),
            status: "todo".to_string(),
            priority: "high".to_string(),
            confidence: 0.87,
            source_text: "we should ".to_string(),
        }
    }

    #[test]
    fn csv_escape_leaves_plain_fields_untouched() {
        assert_eq!(csv_escape("hello world"), "hello world");
        assert_eq!(csv_escape(""), "");
        assert_eq!(csv_escape("2026-07-24"), "2026-07-24");
    }

    #[test]
    fn csv_escape_quotes_commas_quotes_and_newlines() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
        assert_eq!(csv_escape("carriage\rreturn"), "\"carriage\rreturn\"");
        // A field with both a comma AND a quote: quoted + inner quotes doubled.
        assert_eq!(csv_escape("a,\"b\""), "\"a,\"\"b\"\"\"");
    }

    #[test]
    fn to_csv_writes_header_then_escaped_rows() {
        let csv = to_csv(&[
            item("Send budget, revised", "spk_1"),
            item("Book \"the\" venue", ""),
        ]);
        let lines: Vec<&str> = csv.split("\r\n").collect();
        assert_eq!(lines[0], CSV_HEADER);
        assert_eq!(
            lines[1],
            "\"Send budget, revised\",spk_1,2026-07-24,todo,high,0.87,we should "
        );
        assert_eq!(
            lines[2],
            "\"Book \"\"the\"\" venue\",,2026-07-24,todo,high,0.87,we should "
        );
        // Trailing CRLF leaves an empty final element.
        assert_eq!(lines[3], "");
    }

    #[test]
    fn to_csv_of_empty_is_header_only() {
        assert_eq!(
            to_csv(&[]),
            "text,owner,due_at,status,priority,confidence,source_text\r\n"
        );
    }

    #[test]
    fn to_json_round_trips_fields() {
        let json = to_json(&[item("do it", "spk_2")]).unwrap();
        assert!(json.contains("\"text\": \"do it\""));
        assert!(json.contains("\"owner\": \"spk_2\""));
        assert!(json.contains("\"confidence\": 0.87"));
        assert!(json.contains("\"source_text\": \"we should \""));
    }
}
