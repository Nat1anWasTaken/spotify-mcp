//! The tool registry: one MCP tool per [`spotify::Account`] operation.

use std::{future::Future, sync::LazyLock};

use futures::future::BoxFuture;
use rmcp::model::{JsonObject, Tool as Definition, ToolAnnotations};
use schemars::JsonSchema;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use spotify::{Account, types::*};

use super::output::{IntoOutput, Output};

/// Whether a tool only reads or also changes something. Clients use the MCP
/// annotations derived from it to decide what to confirm with the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Read,
    Write,
}

/// What a tool does to the account; determines its [`Kind`] and MCP annotations.
#[derive(Clone, Copy)]
enum Effect {
    Read,
    /// Creates something new on every call (a post, a message).
    Additive,
    /// Changes state; repeating the call changes nothing more.
    Idempotent,
    /// Replaces or finalizes something the user cannot take back.
    Destructive,
}

/// A tool. Serializes as the dashboard's `ToolInfo`.
#[derive(Serialize)]
pub struct Tool {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    /// The Web API area it belongs to (`spotify::ops` module), for grouping
    /// in the dashboard.
    pub group: &'static str,
    #[serde(skip)]
    definition: Definition,
    #[serde(skip)]
    parse: Box<Parse>,
}

/// Parses tool arguments into a request.
type Parse = dyn Fn(&Value) -> serde_json::Result<Box<dyn Call>> + Send + Sync;

/// A tool request with valid arguments.
pub trait Call: Send + Sync {
    /// The request as the call log shows it (before the log bounds its size).
    fn log_arguments(&self) -> Value;

    fn run(self: Box<Self>, account: Account) -> BoxFuture<'static, spotify::Result<Output>>;
}

struct Prepared<R, F> {
    request: R,
    run: F,
}

impl<R, F, Fut, T> Call for Prepared<R, F>
where
    R: Serialize + Send + Sync + 'static,
    F: FnOnce(Account, R) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = spotify::Result<T>> + Send + 'static,
    T: IntoOutput,
{
    fn log_arguments(&self) -> Value {
        serde_json::to_value(&self.request).unwrap_or_default()
    }

    fn run(self: Box<Self>, account: Account) -> BoxFuture<'static, spotify::Result<Output>> {
        let Self { request, run } = *self;
        Box::pin(async move { Ok(run(account, request).await?.into_output()) })
    }
}

impl Tool {
    fn new<R, F, Fut, T>(
        group: &'static str,
        name: &'static str,
        effect: Effect,
        title: &'static str,
        description: &'static str,
        run: F,
    ) -> Self
    where
        R: DeserializeOwned + Serialize + JsonSchema + Send + Sync + 'static,
        F: Fn(Account, R) -> Fut + Copy + Send + Sync + 'static,
        Fut: Future<Output = spotify::Result<T>> + Send + 'static,
        T: IntoOutput,
    {
        Self {
            name,
            title,
            description,
            group,
            kind: match effect {
                Effect::Read => Kind::Read,
                _ => Kind::Write,
            },
            definition: definition::<R>(name, title, description, effect.annotations(title)),
            parse: Box::new(move |args| {
                let request = R::deserialize(args)?;
                Ok(Box::new(Prepared { request, run }))
            }),
        }
    }

    pub fn definition(&self) -> Definition {
        self.definition.clone()
    }

    pub fn parse(&self, args: &Value) -> serde_json::Result<Box<dyn Call>> {
        (self.parse)(args)
    }
}

impl Effect {
    fn annotations(self, title: &str) -> ToolAnnotations {
        let annotations = ToolAnnotations::with_title(title).open_world(true);
        match self {
            Self::Read => annotations.read_only(true),
            write => annotations
                .read_only(false)
                .destructive(matches!(write, Self::Destructive))
                .idempotent(matches!(write, Self::Idempotent)),
        }
    }
}

fn definition<R: JsonSchema + 'static>(
    name: &'static str,
    title: &str,
    description: &'static str,
    annotations: ToolAnnotations,
) -> Definition {
    Definition::new(name, description, JsonObject::new())
        .with_title(title)
        .with_input_schema::<R>()
        .with_annotations(annotations)
}

static TOOLS: LazyLock<Vec<Tool>> = LazyLock::new(|| {
    use Effect::*;
    vec![
        Tool::new(
            "users",
            "get_me",
            Read,
            "Get my profile",
            "The user's Spotify profile: ID, display name, country, product (premium or free) \
             and follower count.",
            |a: Account, r: NoParams| async move { a.get_me(r).await },
        ),
        Tool::new(
            "player",
            "play",
            Idempotent,
            "Start or resume playback",
            "Starts or resumes playback on the active device or `device_id`: an album, artist, \
             playlist or show by `context_uri` (optionally from `offset`), or a list of track \
             and episode `uris`. Without either it resumes what was paused. Needs Premium.",
            |a: Account, r: PlayRequest| async move { a.play(r).await },
        ),
    ]
});

pub fn all() -> impl Iterator<Item = &'static Tool> {
    TOOLS.iter()
}

pub fn find(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|tool| tool.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotations_match_effects() {
        let definition = |name| find(name).unwrap().definition();
        let read = definition("get_me").annotations.unwrap();
        assert_eq!(read.read_only_hint, Some(true));
        let play = definition("play").annotations.unwrap();
        assert_eq!(play.read_only_hint, Some(false));
        assert_eq!(play.destructive_hint, Some(false));
        assert_eq!(play.idempotent_hint, Some(true));
    }

    #[test]
    fn every_tool_has_an_object_schema() {
        let mut names: Vec<_> = all().map(|tool| tool.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all().count(), "tool names are unique");
        for tool in all() {
            let definition = tool.definition();
            assert_eq!(definition.input_schema["type"], "object", "{}", tool.name);
            assert_eq!(definition.title.as_deref(), Some(tool.title));
        }
    }
}
