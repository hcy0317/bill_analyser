"""
解析器工厂模块
负责根据文件类型选择合适的解析器
"""

import logging
import os

# 设置日志记录器
logger = logging.getLogger(__name__)


def get_enhanced_parser(file_path: str, content_preview: str = ""):
    """根据文件路径和内容预览获取对应的增强解析器"""
    filename = os.path.basename(file_path).lower()

    # 优先检查银行账单 - 使用新的银行解析器工厂
    try:
        from . import BankParserFactory
        from .enhanced_bill_parser import BankParserWrapper

        bank_factory = BankParserFactory()
        bank_parser = bank_factory.get_parser(file_path)
        if bank_parser:
            # 如果找到银行解析器，包装成EnhancedBillParser
            return BankParserWrapper(bank_parser)
    except Exception as e:
        logger.warning("银行解析器检测失败: %s", e)

    # 支付宝
    if "alipay" in filename or "支付宝" in filename or "alipay_record" in filename:
        from .enhanced_bill_parser import EnhancedAlipayParser

        return EnhancedAlipayParser()

    # 微信
    if "wechat" in filename or "微信" in filename or "微信支付账单" in filename:
        from .enhanced_bill_parser import EnhancedWechatParser

        return EnhancedWechatParser()

    # 通过内容判断
    if content_preview:
        content_lower = content_preview.lower()

        if "alipay" in content_lower or "支付宝" in content_preview:
            from .enhanced_bill_parser import EnhancedAlipayParser

            return EnhancedAlipayParser()

        if "wechat" in content_lower or "微信支付" in content_preview:
            from .enhanced_bill_parser import EnhancedWechatParser

            return EnhancedWechatParser()

    # 默认根据文件扩展名选择
    if file_path.endswith((".xlsx", ".xls")):
        # Excel文件，尝试银行解析器
        try:
            from . import BankParserFactory
            from .enhanced_bill_parser import BankParserWrapper

            bank_factory = BankParserFactory()
            bank_parser = bank_factory.get_parser(file_path)
            if bank_parser:
                return BankParserWrapper(bank_parser)
        except Exception:
            pass
        # 如果银行解析器失败，使用支付宝解析器作为后备
        from .enhanced_bill_parser import EnhancedAlipayParser

        return EnhancedAlipayParser()

    # CSV文件，可能是支付宝或微信，使用支付宝解析器作为默认
    from .enhanced_bill_parser import EnhancedAlipayParser

    return EnhancedAlipayParser()
