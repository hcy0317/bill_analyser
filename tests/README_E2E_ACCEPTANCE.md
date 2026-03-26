# E2E Acceptance Checklist

正式可重复执行脚本：

- [tests/e2e_acceptance_check.py](tests/e2e_acceptance_check.py)

## 用法

在项目根目录执行：

```powershell
.\.venv\Scripts\python tests\e2e_acceptance_check.py
```

指定账号/地址：

```powershell
.\.venv\Scripts\python tests\e2e_acceptance_check.py --base-url http://127.0.0.1:5000 --username comprehensive_test --password Test@123456
```

若遇到 `Invalid salt`（历史数据哈希格式不一致）可自动修复：

```powershell
.\.venv\Scripts\python tests\e2e_acceptance_check.py --repair-invalid-salt
```

## 验收项

1. 登录并获取 Token
2. 汇率接口包含 `CNY=1.0`
3. 统计时间筛选（今年/去年/全部）参数透传正确
4. 分类接口可用且关键词字段有数据

## 退出码

- `0`：全部通过
- `2`：有失败项
- `1`：运行时错误（如后端不可用）
