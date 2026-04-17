"""
幽灵账单检测和修复工具

用途：
1. 检测source_account_id或destination_account_id为0的账单
2. 检测main_category为None的账单
3. 提供修复建议或自动清理选项
"""
import sqlite3
import sys
from datetime import datetime

from tests.runtime_paths import get_test_db_path


def _resolve_db_path() -> str:
    """Resolve the DB path for this diagnostic script without defaulting to runtime data."""
    if len(sys.argv) >= 2 and not sys.argv[1].startswith("--"):
        return sys.argv[1]
    return str(get_test_db_path("test_ghost_bills.db"))

def check_ghost_bills():
    """检测幽灵账单"""
    conn = sqlite3.connect(_resolve_db_path())
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()

    print("\n" + "="*80)
    print("幽灵账单检测报告")
    print("="*80)
    print(f"检测时间: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")

    # 检测1: source_account_id为0的账单
    cursor.execute("""
        SELECT id, date, type, amount, channel, source_account_id, destination_account_id
        FROM bills
        WHERE source_account_id = 0 OR source_account_id IS NULL
        ORDER BY id DESC
    """)
    ghost_source = cursor.fetchall()

    print(f"【问题1】source_account_id为0的账单: {len(ghost_source)}条")
    if ghost_source:
        print("\n详细列表:")
        for bill in ghost_source[:10]:  # 只显示前10条
            print(f"  ID={bill['id']}, 日期={bill['date']}, 类型={bill['type']}, "
                  f"金额={bill['amount']}, 渠道={bill['channel']}")
    print()

    # 检测2: 投资/转账类型但destination_account_id为0
    cursor.execute("""
        SELECT id, date, type, amount, channel, source_account_id, destination_account_id
        FROM bills
        WHERE type IN ('投资', '转账') 
        AND (destination_account_id = 0 OR destination_account_id IS NULL)
        ORDER BY id DESC
    """)
    ghost_dest = cursor.fetchall()

    print(f"【问题2】投资/转账但destination_account_id为0: {len(ghost_dest)}条")
    if ghost_dest:
        print("\n详细列表:")
        for bill in ghost_dest:
            print(f"  ID={bill['id']}, 日期={bill['date']}, 类型={bill['type']}, "
                  f"金额={bill['amount']}, 源账户ID={bill['source_account_id']}")
    print()

    # 检测3: 分类为None的账单
    cursor.execute("""
        SELECT id, date, type, amount, channel, main_category, sub_category
        FROM bills
        WHERE main_category IS NULL OR main_category = ''
        ORDER BY id DESC
    """)
    no_category = cursor.fetchall()

    print(f"【问题3】分类为空的账单: {len(no_category)}条")
    if no_category:
        print("\n详细列表:")
        for bill in no_category[:10]:
            print(f"  ID={bill['id']}, 日期={bill['date']}, 类型={bill['type']}, "
                  f"金额={bill['amount']}, 渠道={bill['channel']}")
    print()

    # 统计账户信息
    cursor.execute("SELECT id, name, balance FROM accounts ORDER BY id")
    accounts = cursor.fetchall()
    print(f"【账户信息】共{len(accounts)}个账户:")
    for acc in accounts:
        print(f"  ID={acc['id']:3d}, 名称={acc['name']:15s}, 余额={acc['balance']:>12.2f}")
    print()

    conn.close()

    return {
        "ghost_source": len(ghost_source),
        "ghost_dest": len(ghost_dest),
        "no_category": len(no_category)
    }


def fix_ghost_bills(auto_fix=False):
    """修复幽灵账单"""
    if not auto_fix:
        print("\n⚠️  修复模式需要手动确认，添加 --fix 参数以自动修复")
        return

    conn = sqlite3.connect(_resolve_db_path())
    cursor = conn.cursor()

    print("\n" + "="*80)
    print("开始修复幽灵账单")
    print("="*80 + "\n")

    # 修复1: 删除旧的无效账单（source_account_id=0且created_at早于2025-11-19）
    cursor.execute("""
        DELETE FROM bills
        WHERE (source_account_id = 0 OR source_account_id IS NULL)
        AND date < '2025-11-19'
    """)
    deleted_old = cursor.rowcount
    print(f"✓ 删除旧的无效账单: {deleted_old}条")

    # 修复2: 为投资/转账类型但destination_account_id=0的账单设置默认值
    # （假设投资到"活期资产"账户ID=120）
    cursor.execute("""
        SELECT id FROM accounts WHERE name = '活期资产' LIMIT 1
    """)
    default_investment_account = cursor.fetchone()
    if default_investment_account:
        default_id = default_investment_account[0]
        cursor.execute("""
            UPDATE bills
            SET destination_account_id = ?, destination_amount = amount
            WHERE type = '投资' 
            AND (destination_account_id = 0 OR destination_account_id IS NULL)
            AND source_account_id > 0
        """, (default_id,))
        fixed_investment = cursor.rowcount
        print(f"✓ 修复投资账单的目标账户: {fixed_investment}条 (设置为'活期资产')")

    # 修复3: 为转账类型但destination_account_id=0的记录标记为待处理
    cursor.execute("""
        UPDATE bills
        SET main_category = '待处理', sub_category = '缺少目标账户'
        WHERE type = '转账'
        AND (destination_account_id = 0 OR destination_account_id IS NULL)
        AND (main_category IS NULL OR main_category = '')
    """)
    marked_transfer = cursor.rowcount
    print(f"✓ 标记缺少目标账户的转账: {marked_transfer}条")

    conn.commit()
    conn.close()

    print("\n修复完成！")


if __name__ == "__main__":
    stats = check_ghost_bills()

    total_issues = sum(stats.values())

    print("="*80)
    print(f"总计问题: {total_issues}条")
    print("="*80)

    if "--fix" in sys.argv:
        fix_ghost_bills(auto_fix=True)
        print("\n重新检测...")
        check_ghost_bills()
    else:
        print("\n💡 运行 'python tests/test_ghost_bills.py --fix' 以自动修复")
