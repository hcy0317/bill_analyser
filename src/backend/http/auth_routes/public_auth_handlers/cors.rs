#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：返回登录预检响应，保持 public auth CORS header 合同不变。
async fn login_options_handler() -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "login_options_handler", "business operation entered");
    auth_options_handler().await
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：返回注册预检响应，供跨域注册请求在浏览器中通过 OPTIONS。
async fn register_options_handler() -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "register_options_handler", "business operation entered");
    auth_options_handler().await
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：返回通用 auth 预检响应，覆盖 refresh 和 token 等公开认证入口。
async fn auth_options_handler() -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "auth_options_handler", "business operation entered");
    StatusCode::NO_CONTENT.into_response()
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：为公开认证路由补充 CORS header，不改变下游 handler 的 response envelope。
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
