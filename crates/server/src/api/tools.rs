use axum::Json;

use crate::{auth::CurrentUser, mcp};

pub async fn list(_: CurrentUser) -> Json<Vec<&'static mcp::Tool>> {
    Json(mcp::all_tools().collect())
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, StatusCode};
    use serde_json::json;
    use sqlx::PgPool;

    use crate::testing::{self, dashboard_request, send};

    #[sqlx::test(migrations = "../../migrations")]
    async fn lists_tools_and_validates_session_tools(db: PgPool) {
        let state = testing::state(db);
        let (user_id, cookie) = testing::sign_in(&state, 7).await;
        let session = testing::agent_session(&state, user_id).await;
        let app = crate::app(state);

        let tools = send(
            &app,
            dashboard_request(Method::GET, "/api/tools", &cookie, None),
        )
        .await
        .json();
        assert_eq!(
            tools.as_array().unwrap().len(),
            crate::mcp::all_tools().count()
        );
        assert_eq!(
            tools[0],
            json!({
                "name": "get_me",
                "title": "Get my profile",
                "description": tools[0]["description"],
                "kind": "read",
                "destructive": false,
                "group": "users",
            })
        );

        let uri = format!("/api/sessions/{session}");
        let patch = json!({ "allowed_tools": ["get_me", "no_such_tool"] });
        let request = dashboard_request(Method::PATCH, &uri, &cookie, Some(&patch));
        let response = send(&app, request).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(
            response.json()["message"]
                .as_str()
                .unwrap()
                .contains("no_such_tool")
        );
    }
}
