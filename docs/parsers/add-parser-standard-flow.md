# Add Parser Standard Flow

新增或收紧账单解析器时，以 Rust parser-first 主链为准。

## 主要文件

- `src/backend/parsers/`
- `tests/backend/parsers/fixtures/`
- `src/backend/http/import_routes/mod.rs`
- `tests/backend/core/import_pipeline_contracts.rs`

## 工作流

1. 在 `bill-analyser-parsers` 中新增 parser 或收紧已有 parser；dedicated parser 按来源放在 `src/backend/parsers/dedicated/<ParserName>.rs`，共享 CSV/XLS/HTML helper 放在 `dedicated/common.rs`。
2. 更新 parser registry metadata、parser-local 识别判定、字段 normalization 和 parser tags。
3. 添加 golden fixture，覆盖 provider 识别、金额、时间、账户、收支类型和 tags。
4. 如 HTTP 上传入口受影响，同步更新 import runtime contract。
5. 如前端展示字段受影响，同步检查 `src/web/src/lib/services.ts` 和相关 store。

## 验证

```powershell
cargo test -p bill-analyser-parsers
cargo test -p bill-analyser-core --test import_pipeline_contracts
cd src\web
npm run test:coverage
```

全量验收补：

```powershell
cargo test --workspace
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 35
```
