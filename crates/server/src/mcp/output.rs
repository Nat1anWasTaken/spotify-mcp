//! Turning operation results into MCP tool results.

use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::Value;

use super::log;

/// The result of a Spotify tool call.
pub enum Output {
    Json(Value),
    /// A command that returns nothing, such as `pause`.
    Done,
}

impl Output {
    /// The tool result, and what the call log keeps of it.
    pub fn into_parts(self) -> (CallToolResult, Option<Value>) {
        match self {
            Self::Json(value) => {
                let text = render(&value);
                let logged = log::bounded(value, &text);
                (success(text), Some(logged))
            }
            Self::Done => (success("Done.".into()), None),
        }
    }
}

/// How a successful result reads to the agent: pretty JSON.
pub fn render(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

pub fn success(text: String) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(text)])
}

/// A tool result the agent sees as a failure.
pub fn failure(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

pub trait IntoOutput {
    fn into_output(self) -> Output;
}

impl IntoOutput for Value {
    fn into_output(self) -> Output {
        match self {
            Value::Null => Output::Done,
            value => Output::Json(value),
        }
    }
}

impl IntoOutput for () {
    fn into_output(self) -> Output {
        Output::Done
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn text(result: &CallToolResult) -> &str {
        &result.content[0].as_text().unwrap().text
    }

    #[test]
    fn json_results_render_for_agents() {
        let (result, logged) = json!({ "id": 1 }).into_output().into_parts();
        assert_eq!(result.is_error, Some(false));
        assert_eq!(text(&result), "{\n  \"id\": 1\n}");
        assert_eq!(logged, Some(json!({ "id": 1 })));
        assert_eq!(failure("nope").is_error, Some(true));
    }

    #[test]
    fn empty_results_read_done() {
        for output in [Value::Null.into_output(), ().into_output()] {
            let (result, logged) = output.into_parts();
            assert_eq!(text(&result), "Done.");
            assert_eq!(logged, None);
        }
    }
}
