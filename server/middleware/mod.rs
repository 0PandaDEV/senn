use http::{
    StatusCode,
    header::{CONTENT_TYPE, HeaderValue},
};
use topcoat::{
    Result,
    context::{Cx, try_context},
    router::{Body, Next, layer, request::uri, response::Response},
};

use crate::{
    app::PageMeta,
    server::utils::{
        auth::{Authed, is_authed},
        http_util::redirect,
        logging::ApiError,
        state::has_user,
    },
};

fn api_error_response(error: &ApiError) -> Response {
    let body = serde_json::json!({
        "statusCode": error.status,
        "message": error.message,
    });
    let mut response = Response::new(Body::from(body.to_string()));
    *response.status_mut() =
        StatusCode::from_u16(error.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

fn error_to_response(error: topcoat::Error) -> Result<Response> {
    match error.downcast_ref::<ApiError>() {
        Some(api) => Ok(api_error_response(api)),
        None => Err(error),
    }
}

#[layer("/")]
pub async fn middleware(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let path = uri(cx).path().to_string();
    let authed = is_authed(cx).await;
    let has_user = has_user().await;
    if path == "/login" && !has_user {
        return Ok(redirect("/setup"));
    }
    if path == "/setup" && has_user {
        return Ok(redirect("/login"));
    }
    let allowed =
        if path == "/api/agent/connect" || path == "/api/auth/login" || path == "/api/auth/setup" {
            true
        } else if path.starts_with("/api/") {
            authed
        } else {
            match try_context::<PageMeta>(cx) {
                Some(meta) => meta.public || authed,
                None => true,
            }
        };
    if !allowed {
        return Ok(if path.starts_with("/api/") {
            api_error_response(&ApiError {
                status: 401,
                message: "Unauthorized".into(),
            })
        } else {
            redirect(if has_user { "/login" } else { "/setup" })
        });
    }
    let result = if authed {
        next.run(&cx.with(Authed), body).await
    } else {
        next.run(cx, body).await
    };
    let revalidate = path.starts_with("/styles/") || path.starts_with("/js/");
    match result {
        Ok(mut response) => {
            if revalidate {
                response
                    .headers_mut()
                    .insert(http::header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            }
            Ok(response)
        }
        Err(error) => error_to_response(error),
    }
}
