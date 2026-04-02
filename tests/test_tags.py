from datetime import datetime

import pytest
from src.core.db import Database


@pytest.mark.asyncio
async def test_tags_crud():
    # 初始化数据库
    db = Database(":memory:")
    await db.init_db()

    # 创建标签
    tag_id = await db.create_tag({"name": "Test Tag", "color": "#FF0000"})
    assert tag_id is not None

    # 获取全部标签
    tags = await db.get_all_tags()
    assert len(tags) == 1
    assert tags[0]["name"] == "Test Tag"
    assert tags[0]["color"] == "#FF0000"

    # 创建账单
    bill_data = {
        "date": "2023-01-01",
        "type": "支出",
        "amount": 100.0,
        "counterparty": "Test Shop",
        "description": "Test Bill",
        "created_at": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        "updated_at": datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    }
    bill_id = await db.create_bill(bill_data)
    assert bill_id is not None

    # 为账单添加标签
    await db.add_tags_to_bill(bill_id, [tag_id])

    # 获取账单的标签
    bill_tags = await db.get_tags_for_bill(bill_id)
    assert len(bill_tags) == 1
    assert bill_tags[0]["id"] == tag_id

    # 更新账单标签
    # 再创建一个标签
    tag_id_2 = await db.create_tag({"name": "Test Tag 2", "color": "#00FF00"})
    await db.update_bill_tags(bill_id, [tag_id_2])

    # 验证更新结果
    bill_tags = await db.get_tags_for_bill(bill_id)
    assert len(bill_tags) == 1
    assert bill_tags[0]["id"] == tag_id_2

    # 测试批量获取标签
    bill_data_2 = bill_data.copy()
    bill_id_2 = await db.create_bill(bill_data_2)
    assert bill_id_2 is not None
    await db.add_tags_to_bill(bill_id_2, [tag_id, tag_id_2])

    tags_map = await db.get_tags_for_bills([bill_id, bill_id_2])
    assert len(tags_map[bill_id]) == 1
    assert len(tags_map[bill_id_2]) == 2

    # 测试按标签筛选查询账单
    filters = {"tag_ids": [tag_id_2]}
    bills = await db.get_bills(filters=filters)
    assert len(bills) == 2 # 两条账单都带有 tag_id_2

    filters = {"tag_ids": [tag_id]}
    bills = await db.get_bills(filters=filters)
    assert len(bills) == 1 # 只有 bill_2 带有 tag_id

    await db.close()
