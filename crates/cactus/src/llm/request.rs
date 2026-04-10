use std::ffi::CString;
use std::path::{Path, PathBuf};

use hypr_llm_types::{MessageContent, MessagePart};
use url::Url;

use crate::error::{Error, Result};

use super::{CompleteOptions, Message, ToolCall};

pub(super) fn serialize_complete_request(
    messages: &[Message],
    options: &CompleteOptions,
) -> Result<PreparedRequest> {
    let native_messages = prepare_messages(messages)?;
    let messages_c = CString::new(serde_json::to_string(&native_messages)?)?;
    let options_c = CString::new(serde_json::to_string(options)?)?;
    Ok(PreparedRequest {
        messages_c,
        options_c,
    })
}

pub fn validate_messages(messages: &[Message]) -> Result<()> {
    prepare_messages(messages).map(|_| ())
}

#[derive(Debug)]
pub(super) struct PreparedRequest {
    pub(super) messages_c: CString,
    pub(super) options_c: CString,
}

#[derive(serde::Serialize)]
struct NativeMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<NativeToolCall>>,
}

#[derive(serde::Serialize)]
struct NativeToolCall {
    function: NativeFunctionCall,
}

#[derive(serde::Serialize)]
struct NativeFunctionCall {
    name: String,
    arguments: serde_json::Value,
}

fn prepare_messages(messages: &[Message]) -> Result<Vec<NativeMessage>> {
    messages.iter().map(prepare_message).collect()
}

fn prepare_message(message: &Message) -> Result<NativeMessage> {
    let mut content = String::new();
    let mut images = Vec::new();
    let audio = prepare_audio_paths(message.audio.as_deref())?;
    let tool_calls = prepare_tool_calls(message.tool_calls.as_deref());

    match &message.content {
        MessageContent::Text(text) => content.push_str(text),
        MessageContent::Parts(parts) => {
            for part in parts {
                match part {
                    MessagePart::Text { text } => content.push_str(text),
                    MessagePart::ImageUrl { image_url } => {
                        if message.role != "user" {
                            return Err(Error::InvalidRequest(
                                "image parts are only supported for user messages".into(),
                            ));
                        }
                        images.push(resolve_image_path(&image_url.url)?);
                    }
                }
            }
        }
    }

    Ok(NativeMessage {
        role: message.role.clone(),
        content,
        name: message.name.clone(),
        images: (!images.is_empty()).then_some(images),
        audio,
        tool_calls,
    })
}

fn prepare_audio_paths(audio: Option<&[String]>) -> Result<Option<Vec<String>>> {
    let Some(audio) = audio else {
        return Ok(None);
    };

    let resolved = audio
        .iter()
        .map(|path| resolve_local_path(path))
        .collect::<Result<Vec<_>>>()?;

    Ok((!resolved.is_empty()).then_some(resolved))
}

fn prepare_tool_calls(tool_calls: Option<&[ToolCall]>) -> Option<Vec<NativeToolCall>> {
    tool_calls.and_then(|tool_calls| {
        let prepared = tool_calls
            .iter()
            .map(|tool_call| NativeToolCall {
                function: NativeFunctionCall {
                    name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                },
            })
            .collect::<Vec<_>>();

        (!prepared.is_empty()).then_some(prepared)
    })
}

fn resolve_image_path(url: &str) -> Result<String> {
    let parsed = Url::parse(url)
        .map_err(|error| Error::InvalidRequest(format!("invalid image URL: {error}")))?;

    if parsed.scheme() != "file" {
        return Err(Error::InvalidRequest(
            "image_url.url must be a local file:// URL".into(),
        ));
    }
    let path = parsed
        .to_file_path()
        .map_err(|_| Error::InvalidRequest("file image URL must resolve to a local path".into()))?;
    validate_local_file_path(&path, "image")?;
    Ok(path.to_string_lossy().into_owned())
}

fn validate_local_file_path(path: &Path, kind: &str) -> Result<()> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        Error::InvalidRequest(format!("{kind} file is not accessible: {error}"))
    })?;
    if !metadata.is_file() {
        return Err(Error::InvalidRequest(format!(
            "{kind} file path must point to a file"
        )));
    }
    std::fs::File::open(path)
        .map_err(|error| Error::InvalidRequest(format!("{kind} file is not readable: {error}")))?;
    Ok(())
}

fn resolve_local_path(value: &str) -> Result<String> {
    let path = match Url::parse(value) {
        Ok(parsed) if parsed.scheme() == "file" => parsed.to_file_path().map_err(|_| {
            Error::InvalidRequest("file media URL must resolve to a local path".into())
        })?,
        Ok(_) => {
            return Err(Error::InvalidRequest(
                "media paths must be local paths or file:// URLs".into(),
            ));
        }
        Err(_) => PathBuf::from(value),
    };

    validate_local_file_path(&path, "audio")?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct TestImageFile {
        path: PathBuf,
    }

    impl TestImageFile {
        fn create() -> Self {
            let path = unique_temp_path("png");
            std::fs::write(&path, b"image").unwrap();
            Self { path }
        }

        fn url(&self) -> String {
            Url::from_file_path(&self.path).unwrap().to_string()
        }
    }

    impl Drop for TestImageFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn unique_temp_path(extension: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "cactus-image-test-{}-{timestamp}.{extension}",
            std::process::id()
        ))
    }

    fn options() -> CompleteOptions {
        CompleteOptions::default()
    }

    #[test]
    fn prepares_messages_with_images_field_for_user_parts() {
        let image = TestImageFile::create();
        let prepared = serialize_complete_request(
            &[Message::user(vec![
                MessagePart::text("Describe"),
                MessagePart::image_url(image.url()),
            ])],
            &options(),
        )
        .unwrap();

        let json = prepared.messages_c.to_str().unwrap();

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json).unwrap(),
            serde_json::json!([
                {
                    "role": "user",
                    "content": "Describe",
                    "images": [image.path.to_string_lossy()]
                }
            ])
        );
    }

    #[test]
    fn rejects_non_file_image_urls() {
        let error = serialize_complete_request(
            &[Message::user(vec![MessagePart::image_url(
                "https://example.com/test.png",
            )])],
            &options(),
        )
        .unwrap_err();

        assert!(matches!(error, Error::InvalidRequest(_)));
        assert!(
            error
                .to_string()
                .contains("image_url.url must be a local file:// URL")
        );
    }

    #[test]
    fn rejects_missing_local_image_files() {
        let missing = unique_temp_path("png");
        let url = Url::from_file_path(&missing).unwrap().to_string();

        let error = serialize_complete_request(
            &[Message::user(vec![MessagePart::image_url(url)])],
            &options(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("image file is not accessible"));
    }

    #[test]
    fn rejects_non_file_paths() {
        let path = unique_temp_path("png");
        std::fs::create_dir(&path).unwrap();

        let error = serialize_complete_request(
            &[Message::user(vec![MessagePart::image_url(
                Url::from_file_path(&path).unwrap().to_string(),
            )])],
            &options(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("must point to a file"));

        std::fs::remove_dir(&path).unwrap();
    }

    #[test]
    fn prepares_native_audio_name_and_tool_calls() {
        let audio = TestImageFile::create();
        let prepared = serialize_complete_request(
            &[Message::assistant("calling tool")
                .with_name("calculator")
                .with_audio(vec![audio.path.to_string_lossy().into_owned()])
                .with_tool_calls(vec![ToolCall::new(
                    "sum",
                    serde_json::json!({ "a": 1, "b": 2 }),
                )])],
            &options(),
        )
        .unwrap();

        let json = prepared.messages_c.to_str().unwrap();

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json).unwrap(),
            serde_json::json!([
                {
                    "role": "assistant",
                    "content": "calling tool",
                    "name": "calculator",
                    "audio": [audio.path.to_string_lossy()],
                    "tool_calls": [
                        {
                            "function": {
                                "name": "sum",
                                "arguments": {
                                    "a": 1,
                                    "b": 2
                                }
                            }
                        }
                    ]
                }
            ])
        );
    }
}
