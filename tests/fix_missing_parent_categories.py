"""
修复缺失父级记录的分类

功能：
1. 扫描数据库，找出所有只有子分类没有父级记录的主分类
2. 为每个缺失的主分类创建父级记录（sub_category=''）
3. 继承第一个子分类的type和priority
4. 生成详细的修复报告
"""
import argparse
import asyncio
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SRC_ROOT = PROJECT_ROOT / "src"
if str(SRC_ROOT) not in sys.path:
    sys.path.insert(0, str(SRC_ROOT))

from bill_analyser.core.db import Database
from bill_analyser.utils.logger import get_logger

logger = get_logger("FixMissingParentCategories")


async def analyze_missing_parents(db: Database) -> list[dict]:
    """分析缺失父级记录的主分类
    
    Returns:
        List[Dict]: 缺失父级的主分类列表，包含统计信息
    """
    logger.info("开始扫描数据库...")

    all_cats = await db.get_all_categories()

    # 按主分类分组
    main_cats = {}
    for cat in all_cats:
        main = cat["main_category"]
        if main not in main_cats:
            main_cats[main] = {
                "parent": None,
                "children": []
            }

        if cat["sub_category"] == "" or cat["sub_category"] is None:
            main_cats[main]["parent"] = cat
        else:
            main_cats[main]["children"].append(cat)

    # 找出缺失父级的
    missing_parents = []
    for main, info in main_cats.items():
        if info["parent"] is None and len(info["children"]) > 0:
            # 获取第一个子分类的信息作为参考
            first_child = info["children"][0]
            missing_parents.append({
                "main_category": main,
                "child_count": len(info["children"]),
                "reference_type": first_child["type"],
                "reference_priority": first_child.get("priority", 0),
                "first_child": first_child
            })

    logger.info(f"发现 {len(missing_parents)} 个缺失父级记录的主分类")

    return missing_parents


async def create_parent_category(db: Database, main_category: str,
                                 ref_type: int, ref_priority: int) -> int:
    """创建父级分类记录
    
    Args:
        db: 数据库实例
        main_category: 主分类名称
        ref_type: 参考类型（从子分类继承）
        ref_priority: 参考优先级
        
    Returns:
        int: 创建的父级分类ID
    """
    parent_data = {
        "type": ref_type,
        "main_category": main_category,
        "sub_category": "",  # 父级分类的标识
        "priority": ref_priority,
        "keywords": "",
        "description": f"{main_category}（父级分类）",
        "icon": "",
        "color": "",
        "hidden": False
    }

    parent_id = await db.create_category(parent_data)
    logger.info(f"创建父级分类: '{main_category}' (ID: {parent_id}, Type: {ref_type})")

    return parent_id


async def fix_missing_parents(db_path: Path, dry_run: bool = False):
    """修复所有缺失的父级记录
    
    Args:
        dry_run: 如果为True，仅模拟不实际修改数据库
    """
    db = Database(str(db_path))

    logger.info("=" * 80)
    logger.info("修复缺失的父级分类记录")
    logger.info("=" * 80)

    if dry_run:
        logger.info("【模拟模式】- 不会实际修改数据库")

    # 分析缺失的父级
    missing = await analyze_missing_parents(db)

    if len(missing) == 0:
        logger.info("✅ 所有主分类都有父级记录，无需修复")
        return

    # 显示待修复列表
    logger.info("\n待修复的主分类:")
    logger.info("-" * 80)
    for item in missing:
        logger.info(
            f"  • {item['main_category']}: "
            f"{item['child_count']}个子分类, "
            f"Type={item['reference_type']}, "
            f"Priority={item['reference_priority']}"
        )

    if dry_run:
        logger.info("\n【模拟模式】以上分类将被创建父级记录（实际未执行）")
        return

    # 执行修复
    logger.info("\n开始修复...")
    logger.info("-" * 80)

    created_parents = []
    failed = []

    for item in missing:
        try:
            parent_id = await create_parent_category(
                db,
                item["main_category"],
                item["reference_type"],
                item["reference_priority"]
            )

            created_parents.append({
                "main_category": item["main_category"],
                "parent_id": parent_id,
                "child_count": item["child_count"]
            })

            logger.info(f"  ✅ 成功创建: {item['main_category']} (ID: {parent_id})")

        except Exception as e:
            logger.error(f"  ❌ 创建失败: {item['main_category']} - {e}")
            failed.append({
                "main_category": item["main_category"],
                "error": str(e)
            })

    # 生成修复报告
    logger.info("\n" + "=" * 80)
    logger.info("修复完成统计")
    logger.info("=" * 80)
    logger.info(f"总计待修复: {len(missing)} 个主分类")
    logger.info(f"成功创建: {len(created_parents)} 个父级记录")
    logger.info(f"失败: {len(failed)} 个")

    if created_parents:
        logger.info("\n成功创建的父级分类:")
        for item in created_parents:
            logger.info(
                f"  ✅ {item['main_category']} (ID: {item['parent_id']}) "
                f"- 包含 {item['child_count']} 个子分类"
            )

    if failed:
        logger.info("\n失败的分类:")
        for item in failed:
            logger.info(f"  ❌ {item['main_category']}: {item['error']}")

    # 验证修复结果
    logger.info("\n验证修复结果...")
    remaining = await analyze_missing_parents(db)

    if len(remaining) == 0:
        logger.info("✅ 所有主分类现在都有父级记录了！")
    else:
        logger.warning(f"⚠️ 仍有 {len(remaining)} 个主分类缺少父级记录")
        for item in remaining:
            logger.warning(f"  • {item['main_category']}")


async def main():
    """主函数"""
    parser = argparse.ArgumentParser(description="修复缺失父级记录的分类")
    parser.add_argument("--db-path", type=Path, required=True, help="目标数据库路径")
    parser.add_argument("--dry-run", "-d", action="store_true", help="仅分析，不实际写入")
    args = parser.parse_args()

    try:
        await fix_missing_parents(args.db_path.resolve(), dry_run=args.dry_run)

        if args.dry_run:
            logger.info("\n" + "=" * 80)
            logger.info("提示：使用以下命令执行实际修复：")
            logger.info(
                f"  .venv\\Scripts\\python tests\\fix_missing_parent_categories.py --db-path {args.db_path.resolve()}"
            )
            logger.info("=" * 80)

        return 0

    except Exception as e:
        logger.error(f"修复过程发生错误: {e}", exc_info=True)
        return 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
