"""交易图片 REST 主链测试。"""

import io
from datetime import datetime

import pytest


@pytest.fixture(scope='module', name='client')
def _client_fixture():
    """创建测试客户端。"""
    import asyncio

    async def init_services():
        from src.api.app import initialize
        await initialize()

    asyncio.run(init_services())

    from src.api.app import app
    app.config['TESTING'] = True

    with app.test_client() as test_client:
        yield test_client


@pytest.fixture(scope='module', name='auth_headers')
def _auth_headers_fixture(client):
    """返回鉴权请求头。"""
    suffix = int(datetime.now().timestamp())
    username = f'test_transaction_pictures_rest_{suffix}'
    password = 'Test123456!'

    register_response = client.post('/api/auth/register', json={
        'username': username,
        'email': f'{username}@example.com',
        'password': password,
        'nickname': username
    })
    assert register_response.status_code in [200, 409], register_response.get_data(as_text=True)

    login_response = client.post('/api/auth/login', json={
        'loginName': username,
        'password': password
    })
    assert login_response.status_code == 200, login_response.get_data(as_text=True)

    token = ((login_response.get_json() or {}).get('result') or {}).get('token')
    assert token
    return {'Authorization': f'Bearer {token}'}


def test_upload_transaction_picture_rest(client, auth_headers):
    """交易图片上传应走新的 REST 主链。"""
    png_bytes = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc```\x00\x00\x00\x04\x00\x01\x0b\xe7\x02\x9d\x00\x00\x00\x00IEND\xaeB`\x82'
    response = client.post(
        '/api/bills/pictures',
        data={
            'picture': (io.BytesIO(png_bytes), 'test.png')
        },
        headers=auth_headers,
        content_type='multipart/form-data'
    )

    assert response.status_code == 200, response.get_data(as_text=True)
    data = response.get_json() or {}
    assert data.get('success') is True
    result = data.get('result') or {}
    assert result.get('pictureId')
    assert str(result.get('originalUrl', '')).startswith('data:image/png;base64,')


def test_remove_unused_transaction_picture_rest(client, auth_headers):
    """交易图片删除应走新的 REST 主链。"""
    png_bytes = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDATx\x9cc```\x00\x00\x00\x04\x00\x01\x0b\xe7\x02\x9d\x00\x00\x00\x00IEND\xaeB`\x82'
    upload_response = client.post(
        '/api/bills/pictures',
        data={
            'picture': (io.BytesIO(png_bytes), 'remove.png')
        },
        headers=auth_headers,
        content_type='multipart/form-data'
    )
    assert upload_response.status_code == 200
    picture_id = ((upload_response.get_json() or {}).get('result') or {}).get('pictureId')
    assert picture_id

    delete_response = client.post(
        '/api/bills/pictures/unused',
        json={'id': picture_id},
        headers=auth_headers
    )
    assert delete_response.status_code == 200, delete_response.get_data(as_text=True)
    data = delete_response.get_json() or {}
    assert data.get('success') is True
    assert data.get('result') is True


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