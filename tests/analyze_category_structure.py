"""
分析分类结构脚本
"""
import sqlite3
import sys

def analyze_category_structure():
    """分析数据库中的分类层级结构"""
    conn = sqlite3.connect('src/data/bills.db')
    cursor = conn.cursor()
    
    # 查询所有分类
    cursor.execute('''
        SELECT id, main_category, sub_category 
        FROM categories 
        ORDER BY main_category, sub_category
    ''')
    results = cursor.fetchall()
    
    # 统计结构
    main_cats = {}
    for cat_id, main, sub in results:
        if main not in main_cats:
            main_cats[main] = {
                'total': 0,
                'parent_id': None,
                'parent_count': 0,
                'children': []
            }
        
        main_cats[main]['total'] += 1
        
        if sub == '':
            main_cats[main]['parent_count'] += 1
            main_cats[main]['parent_id'] = cat_id
        else:
            main_cats[main]['children'].append({
                'id': cat_id,
                'name': sub
            })
    
    # 打印统计
    print("="*80)
    print("分类层级结构分析")
    print("="*80)
    
    # 找出有问题的分类（有子分类但没有父级记录）
    problems = []
    
    for main, stats in sorted(main_cats.items()):
        print(f"\n主分类: {main}")
        print(f"  总记录数: {stats['total']}")
        print(f"  父级记录: {stats['parent_count']}条 (ID: {stats['parent_id']})")
        print(f"  子级记录: {len(stats['children'])}条")
        
        if len(stats['children']) > 0:
            if stats['parent_count'] == 0:
                problems.append({
                    'type': 'no_parent',
                    'main': main,
                    'children_count': len(stats['children'])
                })
                print(f"  ⚠️ 问题: 有{len(stats['children'])}个子分类但没有父级记录")
            elif stats['parent_count'] > 1:
                problems.append({
                    'type': 'multiple_parents',
                    'main': main,
                    'parent_count': stats['parent_count']
                })
                print(f"  ⚠️ 问题: 有{stats['parent_count']}个父级记录（应该只有1个）")
            
            # 显示前5个子分类
            for i, child in enumerate(stats['children'][:5]):
                print(f"    - [{child['id']}] {child['name']}")
            if len(stats['children']) > 5:
                print(f"    ... 还有 {len(stats['children']) - 5} 个子分类")
    
    # 汇总问题
    if problems:
        print("\n" + "="*80)
        print("发现的问题汇总:")
        print("="*80)
        for prob in problems:
            if prob['type'] == 'no_parent':
                print(f"❌ '{prob['main']}': 有{prob['children_count']}个子分类但缺少父级记录")
            elif prob['type'] == 'multiple_parents':
                print(f"⚠️ '{prob['main']}': 有{prob['parent_count']}个父级记录")
    else:
        print("\n✅ 所有分类结构正常")
    
    conn.close()
    
    return len(problems) == 0

if __name__ == '__main__':
    success = analyze_category_structure()
    sys.exit(0 if success else 1)
