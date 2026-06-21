#[tracing::instrument(level = "debug", skip_all)]
async fn login_options_handler() -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "login_options_handler", "business operation entered");
    auth_options_handler().await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn register_options_handler() -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "register_options_handler", "business operation entered");
    auth_options_handler().await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn auth_options_handler() -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "auth_options_handler", "business operation entered");
    StatusCode::NO_CONTENT.into_response()
}

#[tracing::instrument(level = "debug", skip_all)]
async fn auth_cors_middleware(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let origin = request.headers().get(header::ORIGIN).cloned();
    let requested_headers = request
        .headers()
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .cloned();
    let mut response = next.run(request).await;
    apply_auth_cors_headers(origin.as_ref(), response.headers_mut());
    if method == Method::OPTIONS {
        apply_auth_preflight_headers(requested_headers.as_ref(), response.headers_mut());
    }
    response
}
