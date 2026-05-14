"""交易图片 REST 主链测试。"""

import io


def test_upload_transaction_picture_rest(client, auth_headers):
    """交易图片上传已由 Rust runtime 接管，不再注册 Flask sidecar route shell。"""
    png_bytes = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc```\x00\x00\x00\x04\x00\x01\x0b\xe7\x02\x9d\x00\x00\x00\x00IEND\xaeB`\x82'
    response = client.post(
        '/api/bills/pictures',
        data={
            'picture': (io.BytesIO(png_bytes), 'test.png')
        },
        headers=auth_headers,
        content_type='multipart/form-data'
    )

    assert response.status_code in (404, 405), response.get_data(as_text=True)


def test_remove_unused_transaction_picture_rest(client, auth_headers):
    """交易图片清理已由 Rust runtime 接管，不再注册 Flask sidecar route shell。"""
    delete_response = client.post(
        '/api/bills/pictures/unused',
        json={'id': 'remove.png'},
        headers=auth_headers
    )
    assert delete_response.status_code in (404, 405), delete_response.get_data(as_text=True)


def test_transaction_picture_legacy_routes_removed(client, auth_headers):
    """旧交易图片 v1 路径应已不再作为主链使用。"""
    upload_response = client.post(
        '/api/v1/transaction/pictures/upload.json',
        headers=auth_headers
    )
    remove_response = client.post(
        '/api/v1/transaction/pictures/remove_unused.json',
        json={'id': 'fake-picture-id.png'},
        headers=auth_headers
    )

    assert upload_response.status_code == 404
    assert remove_response.status_code == 404
