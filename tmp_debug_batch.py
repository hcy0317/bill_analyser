import asyncio
import time
from pathlib import Path

from bill_analyser.api.app import app, initialize

asyncio.run(initialize(db_path=str(Path("data") / "debug_batch_create.db")))
app.config["TESTING"] = True
client = app.test_client()
login = client.post("/api/auth/login", json={"loginName": "admin", "password": "admin123"})
if login.status_code != 200:
    username = f"debug_batch_{int(time.time())}"
    client.post(
        "/api/auth/register",
        json={
            "username": username,
            "email": f"{username}@example.com",
            "password": "Test123456!",
            "nickname": username,
        },
    )
    login = client.post("/api/auth/login", json={"loginName": username, "password": "Test123456!"})
headers = {"Authorization": f"Bearer {login.get_json()['result']['token']}"}
acc = client.get("/api/accounts/", headers=headers).get_json()["result"]
if not acc:
    acc_id = client.post(
        "/api/accounts/",
        json={
            "name": "debug account",
            "category": 1,
            "type": 1,
            "icon": "1",
            "color": "00ccff",
            "currency": "CNY",
            "balance": 0,
            "comment": "debug",
            "hidden": False,
            "aliases": [],
        },
        headers=headers,
    ).get_json()["result"]["id"]
else:
    acc_id = acc[0]["id"]
cat_result = client.get("/api/categories/", headers=headers).get_json()["result"]
expense = cat_result.get("3") or cat_result.get(3) or []
if not expense:
    cat_id = client.post(
        "/api/categories/",
        json={
            "name": "debug category",
            "parentId": "0",
            "type": 3,
            "comment": "debug",
            "displayOrder": 0,
            "visible": True,
            "keywords": "",
        },
        headers=headers,
    ).get_json()["result"]["id"]
else:
    cat_id = expense[0]["id"]
now_ms = int(time.time() * 1000)
payload = {
    "clientSessionId": "debug-batch",
    "transactions": [
        {
            "type": 3,
            "categoryId": cat_id,
            "time": now_ms,
            "utcOffset": 480,
            "sourceAccountId": acc_id,
            "destinationAccountId": "0",
            "sourceAmount": 1234,
            "destinationAmount": 1234,
            "hideAmount": False,
            "tagIds": [],
            "pictureIds": [],
            "comment": "debug create",
            "clientSessionId": "debug-row-1",
        }
    ],
}
resp = client.post("/api/bills/batch", json=payload, headers=headers)
print(resp.status_code)
print(resp.get_data(as_text=True))
