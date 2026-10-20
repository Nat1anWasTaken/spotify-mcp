//! Permission decisions over a session's settings. Free of I/O so every rule
//! is unit tested.

use std::fmt;

use super::tools::{Kind, Tool};
use crate::sessions::Permissions;

/// Why a call may not run. Displays as the message the agent receives.
#[derive(Debug, PartialEq, Eq)]
pub enum Denial {
    Disabled,
    NotAllowed(&'static str),
    ReadOnly,
}

impl fmt::Display for Denial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("This connection is disabled in the dashboard."),
            Self::NotAllowed(tool) => write!(
                f,
                "The user has not allowed this connection to use {tool}. They can change that \
                 in the dashboard."
            ),
            Self::ReadOnly => f.write_str(
                "This connection is read-only, so it cannot change anything in Spotify or \
                 control playback. The user can allow write tools in the dashboard.",
            ),
        }
    }
}

/// Whether the session may call the tool. `tools/list` shows exactly the
/// tools that pass.
pub fn check_tool(session: &Permissions, tool: &Tool) -> Result<(), Denial> {
    if !session.enabled {
        return Err(Denial::Disabled);
    }
    let allowed = session
        .allowed_tools
        .as_ref()
        .is_none_or(|allowed| allowed.iter().any(|name| name == tool.name));
    if !allowed {
        Err(Denial::NotAllowed(tool.name))
    } else if tool.kind == Kind::Write && session.read_only {
        Err(Denial::ReadOnly)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::find;

    fn session() -> Permissions {
        Permissions {
            enabled: true,
            read_only: false,
            allowed_tools: None,
        }
    }

    #[test]
    fn tool_checks() {
        let read = find("get_me").unwrap();
        let write = find("play").unwrap();
        assert_eq!(check_tool(&session(), read), Ok(()));
        assert_eq!(check_tool(&session(), write), Ok(()));

        let disabled = Permissions {
            enabled: false,
            ..session()
        };
        for tool in [read, write] {
            assert_eq!(check_tool(&disabled, tool), Err(Denial::Disabled));
        }

        let read_only = Permissions {
            read_only: true,
            ..session()
        };
        assert_eq!(check_tool(&read_only, read), Ok(()));
        assert_eq!(check_tool(&read_only, write), Err(Denial::ReadOnly));

        let restricted = Permissions {
            allowed_tools: Some(vec!["play".into()]),
            ..session()
        };
        assert_eq!(check_tool(&restricted, write), Ok(()));
        assert_eq!(
            check_tool(&restricted, read),
            Err(Denial::NotAllowed("get_me"))
        );
    }
}
