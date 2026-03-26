import pytest
import asyncio
from datetime import datetime
from src.core.db import Database

@pytest.mark.asyncio
async def test_tags_crud():
    # Initialize DB
    db = Database(":memory:")
    await db.init_db()

    # Create a tag
    tag_id = await db.create_tag({'name': "Test Tag", 'color': "#FF0000"})
    assert tag_id is not None

    # Get all tags
    tags = await db.get_all_tags()
    assert len(tags) == 1
    assert tags[0]['name'] == "Test Tag"
    assert tags[0]['color'] == "#FF0000"

    # Create a bill
    bill_data = {
        'date': '2023-01-01',
        'type': '支出',
        'amount': 100.0,
        'counterparty': 'Test Shop',
        'description': 'Test Bill',
        'created_at': datetime.now().strftime('%Y-%m-%d %H:%M:%S'),
        'updated_at': datetime.now().strftime('%Y-%m-%d %H:%M:%S')
    }
    bill_id = await db.create_bill(bill_data)
    assert bill_id is not None

    # Add tag to bill
    await db.add_tags_to_bill(bill_id, [tag_id])

    # Get tags for bill
    bill_tags = await db.get_tags_for_bill(bill_id)
    assert len(bill_tags) == 1
    assert bill_tags[0]['id'] == tag_id

    # Update bill tags
    # Create another tag
    tag_id_2 = await db.create_tag({'name': "Test Tag 2", 'color': "#00FF00"})
    await db.update_bill_tags(bill_id, [tag_id_2])

    # Verify update
    bill_tags = await db.get_tags_for_bill(bill_id)
    assert len(bill_tags) == 1
    assert bill_tags[0]['id'] == tag_id_2

    # Test batch get tags
    bill_data_2 = bill_data.copy()
    bill_id_2 = await db.create_bill(bill_data_2)
    assert bill_id_2 is not None
    await db.add_tags_to_bill(bill_id_2, [tag_id, tag_id_2])

    tags_map = await db.get_tags_for_bills([bill_id, bill_id_2])
    assert len(tags_map[bill_id]) == 1
    assert len(tags_map[bill_id_2]) == 2

    # Test query bills with tag filter
    filters = {'tag_ids': [tag_id_2]}
    bills = await db.get_bills(filters=filters)
    assert len(bills) == 2 # Both bills have tag_id_2

    filters = {'tag_ids': [tag_id]}
    bills = await db.get_bills(filters=filters)
    assert len(bills) == 1 # Only bill_2 has tag_id

    await db.close()
