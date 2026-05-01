"""Same-batch and cross-batch transfer pairing."""
# pylint: disable=broad-exception-caught,duplicate-code,line-too-long
# pylint: disable=too-many-branches,too-many-locals,too-many-return-statements
# pylint: disable=too-many-statements,unused-variable

from datetime import timedelta
from typing import Any

from ...utils.logger import log_method


# pylint: disable=too-few-public-methods


class TransferPairingMixin:

    """TransferPairingMixin implementation shard."""



    def _find_transfer_pairs(self, bills: list[dict[str, Any]]) -> list[tuple[dict, dict]]:
        """识别账户间转账

        v6.57优化: 使用pandas进行初步筛选，减少双重循环开销

        转账配对条件：
        - 时间接近（30秒内）
        - 金额绝对值相等，符号相反（一正一负）
        - 来自不同账户

        处理结果：
        - 金额为负的是转出账户(source_account)
        - 金额为正的是转入账户(destination_account)
        - 两条账单都设置type为'转账'
        - 转出账单的destination_account_id设为转入账户
        - 转入账单的source_account_id设为转出账户

        Args:
            bills: 账单列表

        Returns:
            List[Tuple[Dict, Dict]]: 转账配对列表，每对为(转出账单, 转入账单)
        """
        pairs: list[tuple[dict, dict]] = []

        # 过滤活跃账单（未被移除且金额不为0）
        active_bills = [b for b in bills if not b.get("_removed") and abs(float(b.get("amount", 0))) > 0.001]
        # 添加临时索引
        for i, b in enumerate(active_bills):
            b["_temp_idx"] = i

        n = len(active_bills)
        self.logger.debug("[转账配对] 活跃账单数: %d", n)

        if n < 2:
            return pairs

        # v6.57: 使用pandas构建DataFrame
        df = self._bills_to_dataframe(active_bills)
        df = df[df["datetime"].notna()].copy()

        if len(df) < 2:
            return pairs

        # 分离正负金额账单（转账需要一正一负）
        df_positive = df[df["amount"] >= 0].copy()
        df_negative = df[df["amount"] < 0].copy()

        if df_positive.empty or df_negative.empty:
            self.logger.debug("[转账配对] 无正/负金额配对候选")
            return pairs

        # v6.57: 使用向量化查找时间接近的配对
        time_close_pairs = self._find_time_close_pairs_vectorized(df_positive, df_negative, self.TIME_TOLERANCE)

        if time_close_pairs.empty:
            self.logger.debug("[转账配对] 无时间接近的配对")
            return pairs

        # 合并原始数据
        # v6.60: 使用 _source_identifier 判断来源（优先使用 _parser_id，否则用 source_account_id）
        # 这确保无论是阶段2导入场景（有 _parser_id）还是测试场景（只有 source_account_id）
        # 都能正确识别不同来源的账单
        time_close_pairs = time_close_pairs.merge(
            df_positive[["_idx", "amount", "_source_identifier"]].rename(
                columns={"_idx": "_idx_1", "amount": "amt_pos", "_source_identifier": "src_pos"}
            ),
            on="_idx_1",
        ).merge(
            df_negative[["_idx", "amount", "_source_identifier"]].rename(
                columns={"_idx": "_idx_2", "amount": "amt_neg", "_source_identifier": "src_neg"}
            ),
            on="_idx_2",
        )

        if time_close_pairs.empty:
            return pairs

        # 条件2: 来源不同（不同银行/平台的账单）
        # v6.60: 当两个账单的来源标识都为空时，不认为它们来源不同
        # 只有当 src_pos != src_neg 且两者都非空时才配对
        time_close_pairs = time_close_pairs[
            (time_close_pairs["src_pos"] != time_close_pairs["src_neg"])
            & (time_close_pairs["src_pos"] != "")
            & (time_close_pairs["src_neg"] != "")
        ]

        if time_close_pairs.empty:
            return pairs

        # 条件3: 金额绝对值相等
        time_close_pairs["abs_amt_pos"] = time_close_pairs["amt_pos"].abs()
        time_close_pairs["abs_amt_neg"] = time_close_pairs["amt_neg"].abs()
        time_close_pairs["amount_match"] = (
            time_close_pairs["abs_amt_pos"] - time_close_pairs["abs_amt_neg"]
        ).abs() <= self.AMOUNT_TOLERANCE
        matched_pairs = time_close_pairs[time_close_pairs["amount_match"]]

        if matched_pairs.empty:
            return pairs

        self.logger.debug("[转账配对] 候选匹配数: %d", len(matched_pairs))

        # 处理匹配结果
        matched_indices: set[int] = set()

        for _, row in matched_pairs.iterrows():
            pos_idx = int(row["_idx_1"])
            neg_idx = int(row["_idx_2"])

            if pos_idx in matched_indices or neg_idx in matched_indices:
                continue

            incoming_bill = active_bills[pos_idx]  # 正金额 = 转入
            outgoing_bill = active_bills[neg_idx]  # 负金额 = 转出

            if incoming_bill.get("_removed") or outgoing_bill.get("_removed"):
                continue

            matched_indices.add(pos_idx)
            matched_indices.add(neg_idx)

            # v6.62: 转账配对只保留一条记录（转出账单）
            # 规格：
            # - preview_source_account_id = 负金额账单的 parser_account_id（账户匹配阶段设置）
            # - preview_destination_account_id = 正金额账单的 parser_account_id（账户匹配阶段设置）
            # 转入账单（正金额）标记为已移除，不写入预览表

            # 获取模板ID用于合并
            outgoing_template_id = outgoing_bill.get("_template_id")
            incoming_template_id = incoming_bill.get("_template_id")

            # 设置转出账单为转账类型
            outgoing_bill["type"] = "转账"
            outgoing_bill["_dedup_type"] = "transfer"

            # 合并模板ID：转出账单包含转入账单的模板ID
            outgoing_merged = outgoing_bill.get("_merged_template_ids", [])
            if incoming_template_id:
                outgoing_merged.append(incoming_template_id)
            outgoing_bill["_merged_template_ids"] = outgoing_merged

            # v6.62: 记录转入账单的解析器信息，用于账户匹配阶段设置目标账户
            # 因为账户匹配是在去重之后执行，此时 source_account_id 可能为空
            # 所以记录 parser_id 和 payment_method，让账户匹配阶段能够找到正确的目标账户
            outgoing_bill["_transfer_pair_order"] = "outgoing_first"
            outgoing_bill["_transfer_pair_sources"] = [
                self._build_transfer_source_snapshot(outgoing_bill, role="outgoing"),
                self._build_transfer_source_snapshot(incoming_bill, role="incoming"),
            ]
            outgoing_bill["_destination_parser_id"] = incoming_bill.get("_parser_id", "")
            outgoing_bill["_destination_payment_method"] = incoming_bill.get("payment_method", "")
            outgoing_bill["_destination_counterparty"] = incoming_bill.get("counterparty", "")
            outgoing_bill["_destination_account_id"] = incoming_bill.get("source_account_id")
            outgoing_bill["_destination_account_name"] = (
                incoming_bill.get("account_name")
                or incoming_bill.get("source_account_name")
                or incoming_bill.get("account")
                or incoming_bill.get("payment_method", "")
            )

            # v6.62: 合并描述信息
            outgoing_desc = outgoing_bill.get("description", "") or ""
            incoming_desc = incoming_bill.get("description", "") or ""
            if incoming_desc and incoming_desc not in outgoing_desc:
                merged_desc = f"{outgoing_desc} | {incoming_desc}".strip(" |")
                outgoing_bill["description"] = merged_desc

            # v6.62: 转入账单也设置类型为转账（测试需要验证 transfer_pairs 中两个账单的类型）
            incoming_bill["type"] = "转账"

            # v6.62: 将转入账单标记为已移除（避免重复写入预览表）
            incoming_bill["_removed"] = True
            incoming_bill["_merged_into"] = outgoing_template_id

            amt = abs(float(outgoing_bill.get("amount", 0)))
            # v6.62: 简化日志输出
            self.logger.debug(
                "[转账配对] 金额=%.2f, 转出=%s -> 转入=%s",
                amt,
                outgoing_bill.get("_parser_id", "") or outgoing_bill.get("source_account_id", ""),
                incoming_bill.get("_parser_id", "") or incoming_bill.get("source_account_id", ""),
            )

            pairs.append((outgoing_bill, incoming_bill))

        # 清理临时索引
        for b in active_bills:
            if "_temp_idx" in b:
                del b["_temp_idx"]

        return pairs

    @log_method
    async def _find_cross_batch_transfer_pairs(
        self, bills: list[dict[str, Any]], db, user_id: int = 1, time_tolerance_seconds: int = 300
    ) -> list[tuple[dict, dict]]:
        """检测新导入账单与数据库已有账单之间的转账关系

        当前批次有一笔-100元(支出)的账单，数据库中已有一笔+100元(收入)
        的账单且时间接近来源不同，则识别为跨批次转账配对。

        配对成功后会更新数据库中已有账单的type为'转账'并设置目标/来源账户。

        Args:
            bills: 待导入的账单列表
            db: 数据库实例
            user_id: 用户ID
            time_tolerance_seconds: 时间容差（秒），默认5分钟

        Returns:
            List[Tuple[Dict, Dict]]: 转账对列表 [(新账单, 已有账单), ...]
        """
        pairs: list[tuple[dict, dict]] = []

        active_bills = [b for b in bills if not b.get("_removed", False)]
        if not active_bills:
            return pairs

        # 获取日期范围
        dates = []
        for bill in active_bills:
            dt = self._parse_datetime(bill.get("date", ""))
            if dt:
                dates.append(dt)
        if not dates:
            return pairs

        min_date = min(dates)
        max_date = max(dates)
        start_date = (min_date - timedelta(days=1)).strftime("%Y-%m-%d")
        end_date = (max_date + timedelta(days=1)).strftime("%Y-%m-%d")

        try:
            existing_bills = await db.get_bills_by_date_range(start_date, end_date, user_id=user_id)
        except Exception as e:
            self.logger.error("[跨批次转账] 查询数据库失败: %s", e)
            return pairs

        if not existing_bills:
            return pairs

        # 构建已有账单按 (日期, 金额绝对值) 索引
        existing_index: dict[str, list[dict]] = {}
        for eb in existing_bills:
            abs_amt = abs(float(eb.get("amount", 0)))
            key = f"{eb.get('date', '')[:10]}_{abs_amt:.2f}"
            if key not in existing_index:
                existing_index[key] = []
            existing_index[key].append(eb)

        matched_db_ids = set()

        for bill in active_bills:
            # 跳过已经被标记为转账的账单
            if bill.get("_dedup_type") == "transfer":
                continue

            bill_amt = float(bill.get("amount", 0))
            bill_abs_amt = abs(bill_amt)
            bill_dt = self._parse_datetime(bill.get("date", ""))
            if not bill_dt:
                continue

            bill_source = self._get_source_type(bill)
            key = f"{bill.get('date', '')[:10]}_{bill_abs_amt:.2f}"

            candidates = existing_index.get(key, [])
            for eb in candidates:
                if eb.get("id") in matched_db_ids:
                    continue

                eb_amt = float(eb.get("amount", 0))

                # 金额必须相反（一正一负）
                if not self._amount_opposite(bill_amt, eb_amt):
                    continue

                eb_dt = self._parse_datetime(eb.get("date", ""))
                if not eb_dt:
                    continue

                # 时间必须接近
                if abs((bill_dt - eb_dt).total_seconds()) > time_tolerance_seconds:
                    continue

                eb_source = str(eb.get("source_account_id", "")).lower()
                # 来源必须不同
                if bill_source and eb_source and bill_source == eb_source:
                    continue

                # 配对成功！
                matched_db_ids.add(eb.get("id"))

                # 确定转出/转入方
                if bill_amt < 0:
                    outgoing, incoming_db = bill, eb
                else:
                    outgoing, incoming_db = eb, bill
                    # 新账单是正金额 → 把新账单当做收入方
                    outgoing, incoming_db = eb, bill

                # 更新新账单的转账标记
                bill["type"] = "转账"
                bill["_dedup_type"] = "transfer_cross_batch"
                bill["_cross_batch_db_id"] = eb.get("id")

                # 更新数据库中已有账单（异步更新其type）
                try:
                    await db.update_bill(eb.get("id"), {"type": "转账"}, user_id=user_id)
                except Exception as update_err:
                    self.logger.warning("[跨批次转账] 更新已有账单 ID=%s 失败: %s", eb.get("id"), update_err)

                self.logger.debug(
                    "[跨批次转账] 金额=%.2f, 新账单(%s) ↔ 已有ID=%s", bill_abs_amt, bill_source, eb.get("id")
                )
                pairs.append((bill, eb))
                break  # 每条新账单最多配对一条已有账单

        if pairs:
            self.logger.info("[跨批次转账] 共发现 %d 对跨批次转账", len(pairs))

        return pairs
