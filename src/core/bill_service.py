"""
Bill Service Module - 账单导入服务

异步账单导入、识别、分类和写入数据库的服务模块。
"""

import asyncio
from datetime import datetime
from typing import Dict, List, Any, Optional

from .db import Database
from .category_engine import CategoryEngine
from ..parsers.factory import ParserFactory
from ..utils.logger import get_logger, log_method, log_step
from ..utils.validator import BillValidator
from ..utils.deduplication import DeduplicationEngine, DeduplicationMode


class BillService:
    """账单导入服务"""

    def __init__(
        self,
        db: Optional[Database] = None,
        deduplication_mode: Optional[DeduplicationMode] = None
    ):
        """
        初始化服务

        Args:
            db: 数据库实例，如果为 None 则创建新实例
            deduplication_mode: 去重模式
        """
        self.logger = get_logger('BillService')
        self.db = db or Database()
        self.category_engine = CategoryEngine()
        self.parser_factory = ParserFactory()
        self.validator = BillValidator()
        self.deduplication_engine = DeduplicationEngine(
            deduplication_mode or DeduplicationMode.ADVANCED
        )
        self._initialized = False

    @log_method
    @log_step("初始化账单服务")
    async def initialize(self):
        """初始化服务"""
        if self._initialized:
            return

        # 初始化数据库
        await self.db.init_db()

        # 加载分类规则
        await self.category_engine.load_rules_from_db(self.db)

        self._initialized = True
        self.logger.info("账单服务初始化完成")

    @log_method
    @log_step("导入账单文件")
    async def import_bills(
        self,
        file_path: str,
        parser_type: str = 'auto',
        preview_only: bool = False
    ) -> Dict[str, Any]:
        """
        导入账单文件

        Args:
            file_path: 账单文件路径
            parser_type: 解析器类型（auto/wechat/alipay/icbc/cmbc/abc/ccb）
            preview_only: 是否仅预览，不实际写入数据库

        Returns:
            Dict: 导入结果统计
        """
        if not self._initialized:
            await self.initialize()

        result = {
            'success': False,
            'file': file_path,
            'parser_type': parser_type,
            'preview_only': preview_only,
            'total': 0,
            'valid': 0,
            'invalid': 0,
            'inserted': 0,
            'duplicates': 0,
            'dedup_removed': 0,
            'categories': {},
            'errors': [],
            'preview': []  # 预览数据
        }

        try:
            # 1. 解析文件
            self.logger.info("步骤 1/4: 解析文件 (parser_type=%s)", parser_type)
            loop = asyncio.get_event_loop()

            # 如果指定了parser_type且不是auto，则使用指定的解析器
            if parser_type != 'auto':
                bills = await loop.run_in_executor(
                    None,
                    self.parser_factory.parse,
                    file_path,
                    parser_type
                )
            else:
                bills = await loop.run_in_executor(
                    None,
                    self.parser_factory.parse,
                    file_path
                )

            result['total'] = len(bills)

            if not bills:
                result['errors'].append("文件解析失败或无有效数据")
                return result

            self.logger.info("解析完成: %d 条账单", len(bills))

            # 如果是预览模式，返回所有数据供前端确认
            if preview_only:
                # 转换datetime对象为字符串，防止JSON序列化失败
                preview_bills = []
                for bill in bills:
                    bill_copy = bill.copy()
                    for k, v in bill_copy.items():
                        if isinstance(v, datetime):
                            bill_copy[k] = v.strftime('%Y-%m-%d %H:%M:%S')
                    preview_bills.append(bill_copy)

                result['preview'] = preview_bills
                result['success'] = True
                self.logger.info("预览模式: 返回 %d 条数据", len(bills))
                return result

            # 2. 验证账单
            self.logger.info("步骤 2/4: 验证数据")
            valid_bills, invalid_bills = await loop.run_in_executor(
                None,
                self.validator.validate_bills,
                bills
            )

            result['valid'] = len(valid_bills)
            result['invalid'] = len(invalid_bills)

            if invalid_bills:
                for invalid_bill in invalid_bills[:10]:  # 只记录前10个错误
                    result['errors'].append({
                        'index': invalid_bill.get('_index'),
                        'errors': invalid_bill.get('_validation_errors')
                    })

            if not valid_bills:
                result['errors'].append("没有有效的账单数据")
                return result

            # 3. 去重处理
            self.logger.info("步骤 3/5: 账单去重")
            deduplicated_bills, dedup_stats = await loop.run_in_executor(
                None,
                self.deduplication_engine.deduplicate_all,
                valid_bills
            )

            result['dedup_removed'] = dedup_stats['payment_bank_duplicates'] + \
                                     dedup_stats['transfer_duplicates']

            # 输出去重报告
            dedup_report = self.deduplication_engine.get_deduplication_report(dedup_stats)
            self.logger.info("\n%s", dedup_report)

            # 4. 分类账单
            self.logger.info("步骤 4/5: 自动分类")
            categorized_bills = await self.category_engine.batch_match_categories(deduplicated_bills)

            # 统计分类结果
            for bill in categorized_bills:
                main_cat = bill.get('main_category')
                if main_cat:
                    result['categories'][main_cat] = result['categories'].get(main_cat, 0) + 1

            # 5. 写入数据库
            self.logger.info("步骤 5/5: 写入数据库")
            batch_id = datetime.now().strftime('%Y%m%d%H%M%S')
            inserted = await self.db.insert_bills(categorized_bills, batch_id)

            result['inserted'] = inserted
            result['duplicates'] = result['valid'] - inserted
            result['success'] = True

            self.logger.info(
                "导入完成: 总计 %d 条，有效 %d 条，插入 %d 条，重复 %d 条",
                result['total'],
                result['valid'],
                result['inserted'],
                result['duplicates']
            )

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("导入账单失败: %s", e)
            result['errors'].append(str(e))

        return result

    @log_method
    async def import_multiple_files(self, file_paths: List[str]) -> Dict[str, Any]:
        """
        批量导入多个文件

        Args:
            file_paths: 文件路径列表

        Returns:
            Dict: 汇总结果
        """
        summary = {
            'total_files': len(file_paths),
            'success_files': 0,
            'failed_files': 0,
            'total_bills': 0,
            'inserted_bills': 0,
            'results': []
        }

        self.logger.info("开始批量导入 %d 个文件", len(file_paths))

        for file_path in file_paths:
            result = await self.import_bills(file_path)
            summary['results'].append(result)

            if result['success']:
                summary['success_files'] += 1
                summary['total_bills'] += result['total']
                summary['inserted_bills'] += result['inserted']
            else:
                summary['failed_files'] += 1

        self.logger.info(
            "批量导入完成: 成功 %d 个文件，失败 %d 个文件，共插入 %d 条账单",
            summary['success_files'],
            summary['failed_files'],
            summary['inserted_bills']
        )

        return summary

    @log_method
    @log_step("清理无效账单")
    async def clean_invalid(self) -> int:
        """
        清理无效的账单数据

        Returns:
            int: 清理的数量
        """
        if not self._initialized:
            await self.initialize()

        # 获取所有账单
        bills = await self.db.get_bills()

        invalid_ids = []
        for bill in bills:
            is_valid, _ = self.validator.validate_bill(bill)
            if not is_valid:
                invalid_ids.append(bill['id'])

        # 删除无效账单
        deleted = 0
        for bill_id in invalid_ids:
            if await self.db.delete_bill(bill_id):
                deleted += 1

        self.logger.info("清理完成: 删除了 %d 条无效账单", deleted)
        return deleted

    @log_method
    @log_step("更新账单分类")
    async def update_categories(self, force: bool = False) -> Dict[str, int]:
        """
        重新分类所有账单

        Args:
            force: 是否强制重新分类（包括已有分类的账单）

        Returns:
            Dict: {'total': 总数, 'updated': 更新数}
        """
        if not self._initialized:
            await self.initialize()

        # 获取需要分类的账单
        if force:
            bills = await self.db.get_bills()
        else:
            bills = await self.db.get_bills({'main_category': None})

        self.logger.info("开始重新分类 %d 条账单", len(bills))

        updated = 0
        for bill in bills:
            main_cat, sub_cat = self.category_engine.match_category(bill)

            if main_cat:
                success = await self.db.update_bill(
                    bill['id'],
                    {
                        'main_category': main_cat,
                        'sub_category': sub_cat
                    }
                )

                if success:
                    updated += 1

        self.logger.info("分类更新完成: 总计 %d 条，更新 %d 条", len(bills), updated)

        return {
            'total': len(bills),
            'updated': updated
        }

    @log_method
    async def get_bills(self, filters: Optional[Dict[str, Any]] = None,
                       limit: Optional[int] = None,
                       offset: int = 0) -> List[Dict[str, Any]]:
        """
        查询账单

        Args:
            filters: 过滤条件
            limit: 限制数量
            offset: 偏移量

        Returns:
            List[Dict]: 账单列表
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.get_bills(filters, limit, offset)

    @log_method
    async def delete_bill(self, bill_id: int) -> bool:
        """
        删除账单

        Args:
            bill_id: 账单ID

        Returns:
            bool: 是否成功
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.delete_bill(bill_id)

    @log_method
    async def update_bill(self, bill_id: int, updates: Dict[str, Any]) -> bool:
        """
        更新账单

        Args:
            bill_id: 账单ID
            updates: 更新内容

        Returns:
            bool: 是否成功
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.update_bill(bill_id, updates)

    @log_method
    async def deduplicate(self) -> int:
        """
        去除重复账单

        Returns:
            int: 删除的数量
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.deduplicate()

    @log_method
    async def get_statistics(self) -> Dict[str, Any]:
        """
        获取账单统计信息

        Returns:
            Dict: 统计信息
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.get_statistics()

    async def close(self):
        """关闭服务"""
        await self.db.close()
        self.logger.info("账单服务已关闭")

    async def __aenter__(self):
        """异步上下文管理器入口"""
        await self.initialize()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """异步上下文管理器退出"""
        await self.close()
