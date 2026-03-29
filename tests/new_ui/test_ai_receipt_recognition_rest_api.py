"""AI识图 REST 收口回归测试。"""


def test_ai_receipt_recognition_rest_disabled_safe(client, auth_headers):
    """AI识图新 REST 主链当前应返回未实现或路由未启用。"""
    response = client.post('/api/ml/receipt-recognition', headers=auth_headers)
    assert response.status_code in (404, 501), response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is False
    error_message = f"{data.get('errorMessage') or ''} {data.get('message') or ''}".lower()
    assert 'not implemented' in error_message or 'not found' in error_message


def test_ai_receipt_recognition_legacy_route_removed(client, auth_headers):
    """旧 AI 识图 v1 路径应已移除。"""
    response = client.post('/api/v1/llm/transactions/recognize_receipt_image.json', headers=auth_headers)
    assert response.status_code == 404