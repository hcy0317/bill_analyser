# Agent / Skill / Hook 验活流程

这套流程的目标不是证明“配置文件长得很努力”，而是验证它们是否真的在持续发挥作用。

## 三层验活思路

### 1. 仓库层：检查配置有没有漂移

这一层完全自动化，适合放进 `pytest` 或 CI。

它验证：

- 仓库级入口文件是否齐全：`AGENTS.md`、`CLAUDE.md`、`.github/copilot-instructions.md`、`.codex/AGENTS.md`、`.codex/config.toml`
- `.github/agents` 与 `.cursor/agents` 是否仍然同步
- `.github/skills` 与 `.cursor/skills` 是否仍然同步
- 仓库是否还保持“Cursor hooks 已裁剪”的当前事实
- Codex 的多 agent / MCP 基线是否还在
- Codex / Claude 的仓库专属 skill 入口是否还在

运行方式：

- `\.venv\Scripts\python.exe -m pytest tests/test_reviewer_agent_diff_contract.py tests/test_agent_stack_health.py -v`

通过标准：

- `pytest` 结果全绿
- `scripts/agent_stack_health.py --mode repo` 没有 `FAIL`

### 2. 全局层：检查你机器上的宿主配置是否可用

这一层是**本机检查**，不适合放进公共 CI，因为每个人的 `~/.claude`、`~/.cursor`、`~/.codex` 都不同。

当前脚本会检查：

- `~/.claude/settings.json` 是否存在（Claude hooks 常见入口）
- `~/.cursor/skills-cursor/.cursor-managed-skills-manifest.json` 是否存在
- `~/.codex/config.toml` 是否存在，并能读出模型/MCP 基线
- `~/.codex/skills` 是否有额外自定义内容

运行方式：

- `\.venv\Scripts\python.exe scripts/agent_stack_health.py --mode global`

建议判定：

- `FAIL`：说明关键入口已断，优先修复
- `WARN`：说明“可能没有生效”，需要人工确认
- `INFO`：不是错误，只是说明当前机器没有配置这类全局层

### 3. 行为层：用探针 prompt 验证“真的被命中”

这层最关键，因为很多 agent / skill / hook 的存在并不等于宿主真的加载了它。

运行方式：

- `\.venv\Scripts\python.exe scripts/agent_stack_health.py --mode probes`
- 或直接运行 `\.venv\Scripts\python.exe scripts/agent_stack_health.py --mode all` 查看内置探针列表

重点探针：

1. **reviewer-scope-refusal**
   - 故意不给 `Review Context` 调 reviewer agent
   - 期待它拒绝盲审，并要求 `changed_files` / `diff_text` / `base_ref` / `head_ref`

2. **money-unit-convention**
   - 问“改金额字段最该注意什么”
   - 期待它主动提到元/分转换、REST 主链、async bridge 等仓库规则

3. **python-hook-warning**
   - 临时新建 `.py` 文件，插入 `print()` 或格式问题后保存
   - 期待宿主触发 warning、格式化或类型检查

4. **typescript-hook-warning**
   - 临时新建 `.ts` / `.tsx` 文件，加入 `console.log()` 后保存
   - 期待宿主触发 warning、格式化或类型检查

## 推荐执行节奏

### 日常（每次改 agent/skill/rule/hook 相关文件后）

1. 先跑仓库层：
   - `\.venv\Scripts\python.exe scripts/agent_stack_health.py --mode repo`
2. 再跑全局层：
   - `\.venv\Scripts\python.exe scripts/agent_stack_health.py --mode global`
3. 最后至少做 1 个行为探针：
   - reviewer scope 探针
   - 或金额单位约束探针

### 每次升级宿主或迁移机器后

必须完整跑一次三层流程：

1. 仓库层
2. 全局层
3. 所有行为探针

### 提交 PR 前

如果这次改动涉及以下目录，建议把验活结果贴到 PR 描述里：

- `.github/agents`
- `.github/skills`
- `.github/instructions`
- `.cursor`
- `.claude`
- `.codex`
- `.agents`

建议附带：

- `agent-health` 文本输出
- 是否观察到 hook 真实触发
- 哪个探针通过 / 哪个探针失败

## 当前边界说明

- **仓库层**可以自动化、可回归、适合 CI
- **全局层**可以本机自动检查，但结果因机器而异
- **行为层**目前仍需要人工观察宿主反馈，因为不同 AI 宿主对 prompt 拼装、skills 装载、hooks 触发的可观测性不一致

这不是缺点，反而是现实：

> “文件存在”只能说明你有装备；
> “行为命中”才能说明你真的把装备穿上了。