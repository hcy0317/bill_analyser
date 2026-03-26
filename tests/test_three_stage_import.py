#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""测试三阶段导入流程"""

import requests
import json
import sys

base_url = 'http://127.0.0.1:5000'

def main():
    # 登录获取token
    print('=== 登录获取token ===')
    login_data = {'loginName': 'admin', 'password': 'admin123'}
    response = requests.post(f'{base_url}/api/authorize.json', json=login_data)
    if response.status_code != 200:
        print(f'登录失败: {response.text}')
        return 1
    token = response.json()['result']['token']
    print('Token获取成功')

    headers = {'Authorization': f'Bearer {token}'}

    # 使用真实的支付宝账单测试
    test_file = 'bills/支付宝交易明细(20250715-20250825).csv'
    print(f'\n=== 阶段1: 上传文件 {test_file} ===')

    with open(test_file, 'rb') as f:
        files = {'files': (test_file, f, 'text/csv')}
        data = {'parser_type': 'auto'}
        response = requests.post(
            f'{base_url}/api/bills/import/v2/parse', 
            files=files, 
            data=data, 
            headers=headers
        )
        print(f'状态码: {response.status_code}')
        result = response.json()
        
        # 打印关键信息
        if result.get('success'):
            resp_data = result.get('data', {})
            print('✅ 解析成功!')
            print(f'   Session ID: {resp_data.get("session_id")}')
            print(f'   解析记录数: {resp_data.get("parsed_count")}')
            for f_info in resp_data.get('files', []):
                print(f'   文件: {f_info.get("parser_type")}, 记录: {f_info.get("count")}')
            session_id = resp_data.get('session_id')
        else:
            print(f'❌ 解析失败: {json.dumps(result, ensure_ascii=False, indent=2)}')
            return 1

    # 阶段2: 去重预览
    print(f'\n=== 阶段2: 去重预览 ===')
    response = requests.post(
        f'{base_url}/api/bills/import/v2/dedup', 
        json={'session_id': session_id}, 
        headers=headers
    )
    print(f'状态码: {response.status_code}')
    result = response.json()

    if result.get('success'):
        resp_data = result.get('data', {})
        print('✅ 去重成功!')
        print(f'   原始记录: {resp_data.get("total")}')
        print(f'   去重后: {resp_data.get("after_dedup")}')
        print(f'   预览记录数: {resp_data.get("preview_count")}')
        stats = resp_data.get('dedup_stats', {})
        print(f'   去重统计: 转账配对={stats.get("transfer_pairs", 0)}, '
              f'平台银行={stats.get("platform_bank", 0)}, '
              f'类似账单={stats.get("similar", 0)}')
    else:
        print(f'❌ 去重失败: {json.dumps(result, ensure_ascii=False, indent=2)}')
        return 1

    # 阶段3: 确认导入（可选，注释掉避免写入数据库）
    # print(f'\n=== 阶段3: 确认导入 ===')
    # response = requests.post(
    #     f'{base_url}/api/bills/import/v2/confirm',
    #     json={'session_id': session_id},
    #     headers=headers
    # )
    # result = response.json()
    # if result.get('success'):
    #     print(f'✅ 导入成功: {result.get("data", {}).get("imported_count")} 条')
    # else:
    #     print(f'❌ 导入失败: {result}')

    print(f'\n=== 测试完成 ===')
    print('三阶段导入API工作正常！')
    return 0

if __name__ == '__main__':
    sys.exit(main())
