# Agent Stack Testing

Bill Analyser 的仓库级 agent 资产在 Rust-only 运行态下保持“薄适配器 + 共享 skill”模型。

当前检查重点：

- `AGENTS.md` 是跨工具规则源。
- `.agents/skills/` 保存共享 workflow。
- `.github/`、`.claude/`、`.codex/` 只放平台薄适配器或平台原生 manifest。
- 不重新引入已删除的 sidecar/tooling 入口。

推荐验证：

```powershell
git ls-files *.py
cargo fmt --all -- --check
cargo test --workspace
```

如修改 `.gitea/workflows/ci.yml`，同时运行一次 YAML 解析和本地等价的 Rust/frontend gate。
