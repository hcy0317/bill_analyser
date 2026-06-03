# Matching 域

Matching 域负责正式账单与导入预览中的转账、重复、recurring、learning 和人工反馈。

## Transfer And Duplicate

转账候选要求同日、5 分钟内、金额绝对值在 1 分容差内且方向相反、来源账户不同，并排除已配对或已 suppressed 的账单。重复候选要求金额方向和文本证据满足当前核心约束。

接受 transfer 候选会保留当前账单 id，将两条收支合成为一条转账账单并删除另一条。接受 duplicate 候选会保留当前账单、合并标签并删除候选重复账单。拒绝候选会写入 suppression。

## Rule Center

规则中心的配对总览展示转账配对和重复配对。分类识别、账户识别和周期识别放在“规则配置”二级页，长期学习与 LLM 识别分别独立展示。

## Learning

导入 stage2 先运行确定性规则，再调用 Weaviate recall 生成派生 learning 建议。yellow 建议只展示并等待接受/拒绝；green 建议可自动应用，并保留拒绝入口以降级或抑制。
