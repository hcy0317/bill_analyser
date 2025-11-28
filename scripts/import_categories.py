import asyncio
import json
import sys
import os
from pathlib import Path

# Add project root to path
sys.path.append(str(Path(__file__).parent.parent))

from src.core.db import Database
from src.utils.logger import get_logger
from src.utils.constants import CategoryType

logger = get_logger('ImportCategories')

async def import_categories():
    """Import categories from JSON to DB"""
    try:
        # Initialize DB
        db_path = Path("src/data/bills.db")
        logger.info(f"Connecting to database: {db_path}")
        db = Database(str(db_path))
        await db.init_db()
        
        # Load JSON
        json_path = Path("src/config/categories.json")
        logger.info(f"Loading categories from: {json_path}")
        if not json_path.exists():
            logger.error(f"File not found: {json_path}")
            return

        with open(json_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
            
        # Get existing categories
        all_cats = await db.get_all_categories()
        cat_map = {(c['main_category'], c['sub_category']): c['id'] for c in all_cats}
        logger.info(f"Found {len(all_cats)} existing categories")
        
        updated_count = 0
        created_count = 0
        
        for type_name, main_cats in data.items():
            # Determine type
            if type_name == "收入":
                cat_type = CategoryType.INCOME.value
            elif type_name == "支出":
                cat_type = CategoryType.EXPENSE.value
            elif type_name == "转账":
                cat_type = CategoryType.TRANSFER.value
            elif type_name == "投资":
                cat_type = CategoryType.INVESTMENT.value
            else:
                logger.warning(f"Unknown category type: {type_name}, skipping")
                continue

            for main_cat, sub_cats in main_cats.items():
                # -------------------------------------------------
                # 1. Ensure Parent Category Exists (sub_category="")
                # -------------------------------------------------
                parent_key = (main_cat, "")
                parent_data = {
                    'type': cat_type,
                    'main_category': main_cat,
                    'sub_category': "",
                    'description': f"{main_cat} main category",
                    'priority': 100,
                    'keywords': '',
                    'hidden': False,
                }
                
                if parent_key in cat_map:
                    cat_id = cat_map[parent_key]
                    await db.update_category(cat_id, parent_data)
                    updated_count += 1
                else:
                    logger.info(f"Creating parent category: {main_cat}")
                    await db.create_category(parent_data)
                    created_count += 1

                # -------------------------------------------------
                # 2. Process Sub Categories
                # -------------------------------------------------
                for sub_cat_name, details in sub_cats.items():
                    keywords = details.get('logic_expression', '')
                    
                    cat_data = {
                        'type': cat_type,
                        'main_category': main_cat,
                        'sub_category': sub_cat_name,
                        'description': details.get('description', ''),
                        'priority': details.get('priority', 0),
                        'keywords': keywords,
                        'hidden': not details.get('enabled', True),
                    }
                    
                    key = (main_cat, sub_cat_name)
                    if key in cat_map:
                        cat_id = cat_map[key]
                        logger.info(f"Updating category: {main_cat}/{sub_cat_name} (ID: {cat_id})")
                        await db.update_category(cat_id, cat_data)
                        updated_count += 1
                    else:
                        logger.info(f"Creating category: {main_cat}/{sub_cat_name}")
                        await db.create_category(cat_data)
                        created_count += 1
                    
        logger.info(f"Import completed. Created: {created_count}, Updated: {updated_count}")
        
    except Exception as e:
        logger.error(f"Import failed: {e}", exc_info=True)

if __name__ == "__main__":
    # Set up logging to console
    import logging
    logging.basicConfig(level=logging.INFO)
    
    if sys.platform == 'win32':
        asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())
        
    asyncio.run(import_categories())
