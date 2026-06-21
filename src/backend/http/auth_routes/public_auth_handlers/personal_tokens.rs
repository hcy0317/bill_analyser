#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：生成当前用户 API token，复用 personal token 统一入口并标记 API token 类型。
async fn generate_api_token_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "generate_api_token_handler", "business operation entered");
    generate_personal_token(
        TokenKind::Api,
        state,
        headers,
        connect_info.map(|ConnectInfo(addr)| addr),
        body,
    )
    .await
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：生成当前用户 MCP token，复用 personal token 统一入口并标记 MCP token 类型。
async fn generate_mcp_token_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "generate_mcp_token_handler", "business operation entered");
    generate_personal_token(
        TokenKind::Mcp,
        state,
        headers,
        connect_info.map(|ConnectInfo(addr)| addr),
        body,
    )
    .await
}
