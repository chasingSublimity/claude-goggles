use std::fs;
use std::path::Path;
use serde_json::Value;
use crate::model::TokenUsage;

/// Read a JSONL transcript file and sum all usage fields.
/// Returns None if the file can't be read or contains no usage data.
pub(crate) fn parse_transcript_usage(path: &Path) -> Option<TokenUsage> {
    let content = fs::read_to_string(path).ok()?;
    let mut input_total: u64 = 0;
    let mut output_total: u64 = 0;
    let mut found = false;

    for line in content.lines() {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            if let Some(usage) = v.pointer("/message/usage") {
                if let (Some(inp), Some(out)) = (
                    usage.get("input_tokens").and_then(|v| v.as_u64()),
                    usage.get("output_tokens").and_then(|v| v.as_u64()),
                ) {
                    input_total += inp;
                    output_total += out;
                    found = true;
                }
            }
        }
    }

    if found {
        Some(TokenUsage {
            input: input_total,
            output: output_total,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_transcript(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    #[test]
    fn test_parse_usage_from_transcript() {
        let content = r#"{"type":"assistant","message":{"usage":{"input_tokens":100,"output_tokens":50}}}
{"type":"assistant","message":{"usage":{"input_tokens":200,"output_tokens":100}}}
{"type":"user","message":"hello"}
"#;
        let f = write_temp_transcript(content);
        let usage = parse_transcript_usage(f.path()).unwrap();
        assert_eq!(usage.input, 300);
        assert_eq!(usage.output, 150);
    }

    #[test]
    fn test_parse_empty_transcript() {
        let f = write_temp_transcript("");
        assert!(parse_transcript_usage(f.path()).is_none());
    }

    #[test]
    fn test_parse_no_usage_fields() {
        let content = r#"{"type":"user","message":"hello"}
"#;
        let f = write_temp_transcript(content);
        assert!(parse_transcript_usage(f.path()).is_none());
    }

    #[test]
    fn test_parse_nonexistent_file() {
        assert!(parse_transcript_usage(Path::new("/nonexistent/path.jsonl")).is_none());
    }
}
