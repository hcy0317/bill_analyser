# Bill Analyser 账单分析系统

<p align="center">
  <img src="src/web/public/img/bill_analyser-192.png" alt="Bill Analyser Logo" width="128" height="128">
</p>

<p align="center">
  <strong>多平台账单智能分析与管理系统</strong>
</p>

<p align="center">
  <a href="#功能特性">功能特性</a> •
  <a href="#快速开始">快速开始</a> •
  <a href="#技术栈">技术栈</a> •
  <a href="#项目结构">项目结构</a> •
  <a href="#开发指南">开发指南</a>
</p>

---

## 📋 功能特性

### 账单管理
- ✅ **多平台导入** - 支持微信、支付宝、工商银行、民生银行、建设银行、农业银行等账单导入
- ✅ **智能去重** - 自动识别并处理重复账单（跨平台去重）
- ✅ **智能分类** - 基于关键词的规则引擎，支持 OR/AND/NOT 复杂逻辑
- ✅ **批量操作** - 批量导入、批量编辑、批量删除

### 预算管理
- ✅ **预算设置** - 支持月度、季度、年度预算
- ✅ **执行跟踪** - 实时预算执行率监控
- ✅ **预警提醒** - 可配置的预算预警阈值
- ✅ **层级预算** - 支持一级/二级分类预算

### 统计分析
- ✅ **多维度统计** - 按分类、时间、账户等多维度分析
- ✅ **趋势图表** - ECharts 驱动的可视化图表
- ✅ **资产走势** - 账户资产变化趋势分析

### 多端支持
- ✅ **桌面端** - Vuetify 响应式桌面界面
- ✅ **移动端** - Framework7 移动端适配
- ✅ **多用户** - 数据隔离的多用户支持

---

## 🚀 快速开始

### 环境要求

| 环境 | 版本要求 |
|------|---------|
| Python | 3.10+ |
| Node.js | 18+ (推荐 20+) |
| npm | 8+ |

### 一键部署

#### Windows

```powershell
# 1. 克隆项目
git clone https://github.com/your-username/bill_analyser.git
cd bill_analyser

# 2. 运行安装脚本
.\scripts\install.ps1

# 3. 启动服务
.\启动后端.bat    # 后端服务 (端口 5000)
.\启动前端.bat    # 前端服务 (端口 8081)
```

#### 手动安装

**后端安装:**

```powershell
# 创建虚拟环境
python -m venv .venv

# 激活虚拟环境 (Windows PowerShell)
.\.venv\Scripts\Activate.ps1

# 安装依赖
pip install -r requirements.txt
```

**前端安装:**

```powershell
# 进入前端目录
cd src\web

# 安装依赖
npm install

# 返回项目根目录
cd ..\..
```

### 启动服务

**方式一：使用启动脚本（推荐）**

```powershell
# Windows - 使用 bat 文件
.\启动后端.bat    # 新窗口启动后端
.\启动前端.bat    # 新窗口启动前端

# Windows - 使用 PowerShell 脚本
.\start_backend.ps1
.\start_frontend.ps1
```

**方式二：手动启动**

```powershell
# 终端1 - 启动后端
.\.venv\Scripts\python.exe src\api\app.py

# 终端2 - 启动前端
cd src\web
npm run dev
```

### 访问应用

- **前端界面**: http://127.0.0.1:8081
- **后端 API**: http://127.0.0.1:5000/api
- **健康检查**: http://127.0.0.1:5000/api/health

---

## 🛠 技术栈

### 后端
| 技术 | 说明 |
|------|------|
| Python 3.10+ | 主要开发语言 |
| Flask 3.x | Web 框架 |
| aiosqlite | 异步 SQLite 操作 |
| Pandas | 数据处理 |
| PyJWT | JWT 认证 |
| bcrypt | 密码加密 |

### 前端
| 技术 | 说明 |
|------|------|
| Vue 3 | 前端框架 |
| TypeScript | 类型安全 |
| Vuetify 3 | UI 组件库 (桌面端) |
| Framework7 | UI 组件库 (移动端) |
| ECharts | 图表库 |
| Pinia | 状态管理 |
| Vue Router | 路由管理 |

### 数据库
| 技术 | 说明 |
|------|------|
| SQLite | 轻量级数据库 |
| WAL 模式 | 并发读写优化 |

---

## 📁 项目结构

```
bill_analyser/
├── src/                          # 源代码
│   ├── api/                      # Flask 后端
│   │   ├── app.py               # 应用入口
│   │   ├── routes/              # API 路由
│   │   ├── adapters/            # 数据适配器
│   │   └── middleware/          # 中间件
│   ├── core/                     # 核心业务逻辑
│   │   ├── db.py                # 数据库操作
│   │   ├── bill_service.py      # 账单服务
│   │   └── category_engine.py   # 分类引擎
│   ├── parsers/                  # 账单解析器
│   │   ├── wechat.py            # 微信解析
│   │   ├── alipay.py            # 支付宝解析
│   │   └── ...                  # 其他银行
│   ├── utils/                    # 工具函数
│   │   ├── logger.py            # 日志系统
│   │   └── ...
│   └── web/                      # Vue 前端
│       ├── src/
│       │   ├── views/           # 页面组件
│       │   ├── components/      # 通用组件
│       │   ├── stores/          # Pinia 状态
│       │   ├── models/          # 数据模型
│       │   └── locales/         # 国际化
│       └── package.json
├── config/                       # 配置文件
├── data/                         # 数据文件 (SQLite)
├── logs/                         # 日志文件
├── tests/                        # 测试文件
├── docs/                         # 文档
├── requirements.txt              # Python 依赖
├── start_backend.ps1             # 后端启动脚本
├── start_frontend.ps1            # 前端启动脚本
└── README.md                     # 项目说明
```

---

## 📖 开发指南

### 代码规范

- **Python**: 使用 Pylint 检查，目标评分 ≥ 9.0/10
- **TypeScript**: 使用 ESLint + Vue TSC 检查
- **提交信息**: 遵循 Conventional Commits 规范

### 运行测试

```powershell
# 激活虚拟环境
.\.venv\Scripts\Activate.ps1

# 运行所有测试
python -m pytest tests/ -v --timeout=60

# 运行特定测试
python -m pytest tests/test_db.py -v

# 生成覆盖率报告
python -m pytest tests/ --cov=src --cov-report=html
```

### 代码检查

```powershell
# Python 代码检查
python -m pylint src/core/*.py src/api/routes/*.py

# 前端代码检查
cd src\web
npm run lint
```

### API 文档

主要 API 端点:

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/v1/transactions/list.json` | GET | 获取交易列表 |
| `/api/v1/accounts/list.json` | GET | 获取账户列表 |
| `/api/v1/transaction/categories/list.json` | GET | 获取分类列表 |
| `/api/v1/budgets/list.json` | GET | 获取预算列表 |
| `/api/health` | GET | 健康检查 |

详细 API 文档请参考 `.github/copilot-instructions.md`

---

## 🔧 配置说明

### 环境变量

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `PYTHONPATH` | Python 模块路径 | 项目根目录 |
| `FLASK_ENV` | Flask 环境 | `development` |
| `BILL_ANALYSER_OPERATION_PASSWORD` | 敏感操作密码 | (未设置) |

### 服务器配置

配置文件: `config/server_config.json`

```json
{
  "host": "127.0.0.1",
  "port": 5000,
  "debug": true
}
```

---

## 📝 更新日志

### v6.29 (2025-11-28)
- ✅ 多用户数据隔离
- ✅ 深色主题支持
- ✅ 预算管理优化
- ✅ 标签系统完善

查看完整更新日志: [CHANGELOG.md](docs/CHANGELOG.md)

---

## 🤝 贡献指南

1. Fork 本项目
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送分支 (`git push origin feature/AmazingFeature`)
5. 提交 Pull Request

---

## 📄 许可证

本项目基于 MIT 许可证开源 - 查看 [LICENSE](LICENSE) 文件了解详情

---

## 🙏 致谢

- [ezbookkeeping](https://github.com/mayswind/ezbookkeeping) - 前端架构参考
- [Vue.js](https://vuejs.org/) - 前端框架
- [Flask](https://flask.palletsprojects.com/) - 后端框架
