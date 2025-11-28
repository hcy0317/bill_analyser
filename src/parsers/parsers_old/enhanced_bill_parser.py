"""
增强的账单解析模块 - 重构版本
根据实际账单文件格式优化解析逻辑，支持支付宝、微信、工商银行、中国农业银行等
"""
import os
import re
from abc import ABC, abstractmethod
from datetime import datetime

import pandas as pd

from src.utils.logger import bill_logger
from src.core.unified_category_manager import UnifiedCategoryManager
from . import BankParserFactory

bank_factory = BankParserFactory()


class EnhancedBillParser(ABC):
    """增强的账单解析器基类"""

    def __init__(self):
        """初始化解析器，加载分类管理器"""
        self.category_manager = None
        try:
            # 修复配置文件路径 - 从项目根目录开始
            config_path = os.path.join(os.path.dirname(__file__), '..', '..', 'config', 'master_category_config.json')
            config_path = os.path.abspath(config_path)
            if os.path.exists(config_path):
                self.category_manager = UnifiedCategoryManager(config_path)
            else:
                bill_logger.warning(f"分类配置文件不存在: {config_path}")
        except Exception as e:
            bill_logger.warning(f"无法加载分类管理器: {e}")

    @abstractmethod
    def parse(self, file_path: str) -> pd.DataFrame:
        """解析账单文件"""
        raise NotImplementedError("子类必须实现parse方法")

    def _classify_transaction(self, description: str, merchant: str = '') -> tuple:
        """对交易进行分类"""
        if self.category_manager:
            try:
                # 合并商品说明和交易对方用于分类
                full_description = f"{description} {merchant}".strip()
                return self.category_manager.categorize(full_description)
            except Exception as e:
                bill_logger.warning(f"分类交易时出错: {e}")
        return '其他', '其他'

    def standardize_amount(self, amount_str: str) -> float:
        """标准化金额"""
        if pd.isna(amount_str) or amount_str == '':
            return 0.0

        # 去除非数字字符，保留负号和小数点
        amount_str = str(amount_str).strip()
        # 先移除常见的货币符号
        amount_str = amount_str.replace('¥', '').replace('$', '').replace('￥', '').replace(',', '')
        # 然后移除其他非数字字符，但保留负号和小数点
        cleaned = re.sub(r'[^\d.-]', '', amount_str)

        try:
            return float(cleaned) if cleaned else 0.0
        except ValueError:
            return 0.0

    def standardize_date(self, date_str: str) -> str:
        """标准化日期格式"""
        if pd.isna(date_str) or not date_str:
            return datetime.now().strftime('%Y-%m-%d %H:%M:%S')

        date_str = str(date_str).strip()

        # 如果包含微秒，先移除微秒部分
        if '.' in date_str and re.match(r'\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+', date_str):
            date_str = date_str.split('.')[0]

        # 如果日期字符串已经是标准格式，直接返回
        if re.match(r'^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$', date_str):
            return date_str

        # 尝试各种日期格式
        date_formats = [
            '%Y-%m-%d %H:%M:%S',
            '%Y-%m-%d %H:%M:%S.%f',  # 包含微秒的格式
            '%Y-%m-%d',
            '%Y/%m/%d %H:%M:%S',
            '%Y/%m/%d',
            '%Y%m%d',
            '%m/%d/%Y',
            '%d/%m/%Y'
        ]

        for fmt in date_formats:
            try:
                dt = datetime.strptime(date_str, fmt)
                # 如果原始格式只包含日期，添加00:00:00而不是当前时间
                if fmt in ['%Y-%m-%d', '%Y/%m/%d', '%Y%m%d', '%m/%d/%Y', '%d/%m/%Y']:
                    return dt.strftime('%Y-%m-%d 00:00:00')
                else:
                    return dt.strftime('%Y-%m-%d %H:%M:%S')
            except ValueError:
                continue

        # 如果都失败了，尝试提取日期部分
        date_match = re.search(r'\d{4}-\d{2}-\d{2}', date_str)
        if date_match:
            return date_match.group() + ' 00:00:00'

        # 最后的后备选项，返回当前时间（但记录警告）
        bill_logger.warning(f"无法解析日期格式: {date_str}，使用当前时间")
        return datetime.now().strftime('%Y-%m-%d %H:%M:%S')

    def _standardize_output(self, df: pd.DataFrame, file_path: str) -> pd.DataFrame:
        """标准化输出格式"""
        if df.empty:
            return df

        result_df = df.copy()

        # 确保必要列存在
        required_columns = {
            '日期': lambda: datetime.now().strftime('%Y-%m-%d %H:%M:%S'),
            '商品说明': lambda: '未知交易',
            '交易对方': lambda: '',
            '收支': lambda: '支出',
            '金额': lambda: 0.0,
            '大类': lambda: '其他',
            '小类': lambda: '其他'
        }

        for col, default_func in required_columns.items():
            if col not in result_df.columns:
                result_df[col] = default_func()

        # 标准化数据类型
        try:
            result_df['日期'] = result_df['日期'].apply(self.standardize_date)
            result_df['金额'] = result_df['金额'].apply(self.standardize_amount)
            result_df['商品说明'] = result_df['商品说明'].astype(str).fillna('未知交易')
            result_df['交易对方'] = result_df['交易对方'].astype(str).fillna('')
            result_df['收支'] = result_df['收支'].astype(str).fillna('支出')

            # 使用分类管理器进行分类
            if self.category_manager:
                for idx, row in result_df.iterrows():
                    major_cat, minor_cat = self._classify_transaction(
                        str(row['商品说明']),
                        str(row.get('交易对方', ''))
                    )
                    result_df.at[idx, '大类'] = major_cat
                    result_df.at[idx, '小类'] = minor_cat

        except Exception as e:
            bill_logger.log_parsing_error(file_path, e, {"标准化过程出错": str(e)})

        return result_df


class BankParserWrapper(EnhancedBillParser):
    """银行解析器包装类，将新的银行解析器适配到EnhancedBillParser接口"""

    def __init__(self, bank_parser):
        super().__init__()
        self.bank_parser = bank_parser

    def parse(self, file_path: str) -> pd.DataFrame:
        """使用银行解析器解析文件并标准化输出"""
        try:
            # 使用银行解析器解析
            result_df = self.bank_parser.parse(file_path)

            if result_df.empty:
                return result_df

            # 进行额外的标准化处理
            return self._standardize_output(result_df, file_path)

        except Exception as e:
            bill_logger.log_parsing_error(file_path, e)
            return pd.DataFrame()


class EnhancedAlipayParser(EnhancedBillParser):
    """增强的支付宝解析器 - 重构版本"""

    def parse(self, file_path: str) -> pd.DataFrame:
        """解析支付宝账单文件"""
        try:
            bill_logger.log_file_processing(file_path, "开始解析支付宝账单")

            # 先读取原始文件找到数据开始行
            data_start_line = -1
            with open(file_path, 'r', encoding='gbk') as f:
                lines = f.readlines()
                for i, line in enumerate(lines):
                    if '交易时间' in line and '交易分类' in line and ',' in line:
                        data_start_line = i
                        break

            if data_start_line == -1:
                bill_logger.log_parsing_error(file_path, Exception("未找到支付宝数据表头行"))
                return pd.DataFrame()

            # 从找到的行开始读取CSV数据，表头行作为列名
            df = pd.read_csv(file_path, encoding='gbk', skiprows=data_start_line, header=0, on_bad_lines='skip')

            if df.empty:
                bill_logger.log_parsing_error(file_path, Exception("支付宝数据为空"))
                return pd.DataFrame()

            # 提取数据
            return self._extract_alipay_data(df, file_path)

        except Exception as e:
            bill_logger.log_parsing_error(file_path, e)
            return pd.DataFrame()

    def _extract_alipay_data(self, df: pd.DataFrame, file_path: str) -> pd.DataFrame:
        """提取支付宝数据"""
        try:
            # 清理数据 - 过滤空行和无效行
            df = df.dropna(subset=[df.columns[0]])  # 删除第一列为空的行

            # 过滤有效状态的交易
            if '交易状态' in df.columns:
                # 扩展有效状态列表，包含更多支付宝状态
                valid_statuses = ['交易成功', '支付成功', '还款成功', '退款成功', '转账成功', '充值成功', '付款成功，基金份额确认中', '等待确认收货']
                pattern = '|'.join(valid_statuses)
                df = df[df['交易状态'].astype(str).str.contains(pattern, na=False)]

            # 处理金额 - 支付宝金额在'金额'列
            if '金额' in df.columns:
                df.loc[:, '金额'] = df['金额'].apply(self.standardize_amount)

                # 根据收支类型调整金额符号
                if '收/支' in df.columns:
                    mask_out = df['收/支'].astype(str).str.contains('支出', na=False)
                    mask_in = df['收/支'].astype(str).str.contains('收入', na=False)

                    # 支出金额为负数
                    df.loc[mask_out, '金额'] = -df.loc[mask_out, '金额'].abs()
                    # 收入金额为正数
                    df.loc[mask_in, '金额'] = df.loc[mask_in, '金额'].abs()

            # 标准化列名映射
            column_mapping = {
                '交易时间': '日期',
                '收/支': '收支',
                '金额': '金额',
                '商品说明': '商品说明',
                '交易对方': '交易对方'
            }

            for old_col, new_col in column_mapping.items():
                if old_col in df.columns:
                    df = df.rename(columns={old_col: new_col})

            result_df = self._standardize_output(df, file_path)
            bill_logger.log_file_processing(file_path, f"支付宝账单解析完成，共{len(result_df)}条记录")

            return result_df

        except Exception as e:
            bill_logger.log_parsing_error(file_path, e)
            return pd.DataFrame()


class EnhancedWechatParser(EnhancedBillParser):
    """增强的微信解析器 - 重构版本，支持CSV和XLSX格式"""

    def parse(self, file_path: str) -> pd.DataFrame:
        """解析微信账单文件"""
        try:
            bill_logger.log_file_processing(file_path, "开始解析微信账单")

            # 检查文件类型
            if file_path.lower().endswith('.xlsx'):
                return self._parse_xlsx_format(file_path)
            else:
                return self._parse_csv_format(file_path)

        except Exception as e:
            bill_logger.log_parsing_error(file_path, e)
            return pd.DataFrame()

    def _parse_csv_format(self, file_path: str) -> pd.DataFrame:
        """解析CSV格式的微信账单"""
        # 先读取原始文件找到数据开始行
        data_start_line = -1
        with open(file_path, 'r', encoding='utf-8') as f:
            lines = f.readlines()
            for i, line in enumerate(lines):
                if '交易时间' in line and '交易类型' in line and ',' in line:
                    data_start_line = i
                    break

        if data_start_line == -1:
            bill_logger.log_parsing_error(file_path, Exception("未找到微信数据表头行"))
            return pd.DataFrame()

        # 从找到的行开始读取CSV数据，表头行作为列名
        try:
            df = pd.read_csv(file_path, encoding='utf-8', skiprows=data_start_line, header=0, on_bad_lines='skip')
        except Exception:
            # 如果UTF-8失败，尝试GBK
            df = pd.read_csv(file_path, encoding='gbk', skiprows=data_start_line, header=0, on_bad_lines='skip')

        if df.empty:
            bill_logger.log_parsing_error(file_path, Exception("微信数据为空"))
            return pd.DataFrame()

        return self._extract_wechat_data(df, file_path)

    def _parse_xlsx_format(self, file_path: str) -> pd.DataFrame:
        """解析XLSX格式的微信账单"""
        try:
            # 先读取文件找到数据开始行
            df_raw = pd.read_excel(file_path, header=None)

            # 遍历行来找到表头位置
            data_start_row: int = -1
            for idx, (_, row) in enumerate(df_raw.iterrows()):
                if pd.notna(row[0]) and '交易时间' in str(row[0]):
                    # 检查是否包含其他关键字段
                    row_str = ' '.join([str(x) for x in row if pd.notna(x)])
                    if '交易类型' in row_str and '交易对方' in row_str:
                        data_start_row = idx
                        break

            if data_start_row == -1:
                bill_logger.log_parsing_error(file_path, Exception("未找到微信XLSX数据表头行"))
                return pd.DataFrame()

            # 从找到的行开始重新读取，设置正确的列名
            df = pd.read_excel(file_path, header=data_start_row)

            if df.empty:
                bill_logger.log_parsing_error(file_path, Exception("微信XLSX数据为空"))
                return pd.DataFrame()

            return self._extract_wechat_data(df, file_path)

        except Exception as e:
            bill_logger.log_parsing_error(file_path, e)
            return pd.DataFrame()

    def _extract_wechat_data(self, df: pd.DataFrame, file_path: str) -> pd.DataFrame:
        """提取微信数据"""
        try:
            # 清理数据 - 过滤空行和无效行
            df = df.dropna(subset=[df.columns[0]])  # 删除第一列为空的行

            # 过滤有效状态的交易 - 微信字段是'当前状态'
            if '当前状态' in df.columns:
                # 扩展有效状态列表，包含更多微信状态
                valid_statuses = ['支付成功', '已转账', '已存入零钱', '交易成功', '已到账', '对方已收钱']
                pattern = '|'.join(valid_statuses)
                df = df[df['当前状态'].astype(str).str.contains(pattern, na=False)]

            # 处理金额 - 移除¥符号
            if '金额(元)' in df.columns:
                df['金额'] = df['金额(元)'].astype(str).str.replace('¥', '').apply(self.standardize_amount)
            elif '金额' in df.columns:
                df['金额'] = df['金额'].astype(str).str.replace('¥', '').apply(self.standardize_amount)

            # 根据收支类型调整金额符号
            if '收/支' in df.columns:
                mask_out = df['收/支'].astype(str).str.contains('支出', na=False)
                mask_in = df['收/支'].astype(str).str.contains('收入', na=False)

                # 支出金额为负数
                df.loc[mask_out, '金额'] = -df.loc[mask_out, '金额'].abs()
                # 收入金额为正数
                df.loc[mask_in, '金额'] = df.loc[mask_in, '金额'].abs()

            # 标准化列名映射
            column_mapping = {
                '交易时间': '日期',
                '商品': '商品说明',
                '收/支': '收支',
                '金额': '金额',
                '交易对方': '交易对方',
                '当前状态': '交易状态'
            }

            for old_col, new_col in column_mapping.items():
                if old_col in df.columns:
                    df = df.rename(columns={old_col: new_col})

            result_df = self._standardize_output(df, file_path)
            bill_logger.log_file_processing(file_path, f"微信账单解析完成，共{len(result_df)}条记录")

            return result_df

        except Exception as e:
            bill_logger.log_parsing_error(file_path, e)
            return pd.DataFrame()
