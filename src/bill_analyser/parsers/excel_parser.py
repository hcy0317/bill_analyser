"""
Excel文件解析器

支持.xlsx和.xls格式
"""

from pathlib import Path
from typing import Any, Dict, List, Optional

try:
    import openpyxl
    EXCEL_AVAILABLE = True
except ImportError:
    EXCEL_AVAILABLE = False

from .base_parser import BaseParser, ParserFactory


class ExcelParser(BaseParser):
    """Excel文件解析器"""

    def __init__(self):
        super().__init__()
        if not EXCEL_AVAILABLE:
            raise ImportError("需要安装openpyxl库: pip install openpyxl")

    async def parse(self, file_path: str, config: Optional[Dict[str, Any]] = None) -> List[Dict[str, Any]]:
        """解析Excel文件"""
        config = config or {}
        
        sheet_name = config.get('sheet_name', 0)  # 默认第一个sheet
        skip_rows = config.get('skip_rows', 0)
        has_header = config.get('has_header', True)
        field_mapping = config.get('field_mappings', {})

        bills = []

        wb = openpyxl.load_workbook(file_path, read_only=True, data_only=True)
        
        # 获取工作表
        if isinstance(sheet_name, int):
            sheet = wb.worksheets[sheet_name]
        else:
            sheet = wb[sheet_name]

        rows = list(sheet.iter_rows(values_only=True))
        
        # 跳过行
        rows = rows[skip_rows:]

        if not rows:
            return bills

        # 获取header
        if has_header:
            headers = rows[0]
            data_rows = rows[1:]
        else:
            headers = [f"Column_{i}" for i in range(len(rows[0]))]
            data_rows = rows

        for row_idx, row in enumerate(data_rows):
            try:
                # 构建字典
                raw_data = {}
                for idx, value in enumerate(row):
                    if idx < len(headers):
                        raw_data[headers[idx]] = value

                # 应用字段映射
                if field_mapping:
                    bill = self.standardize_bill(raw_data, field_mapping)
                    bills.append(bill)

            except Exception as e:
                self.logger.warning(f"解析第{row_idx + skip_rows + 2}行失败: {e}")
                continue

        wb.close()
        
        self.logger.info(f"成功解析Excel文件，共{len(bills)}条记录")
        return bills

    async def validate(self, file_path: str) -> bool:
        """验证是否为有效的Excel文件"""
        try:
            path = Path(file_path)
            if not path.exists():
                return False

            if path.suffix.lower() not in ['.xlsx', '.xls']:
                return False

            # 尝试打开文件
            wb = openpyxl.load_workbook(file_path, read_only=True)
            wb.close()
            return True

        except Exception as e:
            self.logger.debug(f"Excel验证失败: {e}")
            return False

    async def detect_encoding(self, file_path: str) -> str:
        """Excel文件不需要编码检测"""
        return 'utf-8'

    async def preview(self, file_path: str, rows: int = 10) -> Dict[str, Any]:
        """预览Excel内容"""
        wb = openpyxl.load_workbook(file_path, read_only=True, data_only=True)
        sheet = wb.worksheets[0]

        all_rows = list(sheet.iter_rows(values_only=True))
        
        if not all_rows:
            wb.close()
            return {
                'headers': [],
                'sample_data': [],
                'sheet_names': wb.sheetnames,
                'total_rows': 0
            }

        headers = all_rows[0] if all_rows else []
        sample_data = all_rows[1:min(rows + 1, len(all_rows))]

        result = {
            'headers': list(headers),
            'sample_data': [list(row) for row in sample_data],
            'sheet_names': wb.sheetnames,
            'total_rows': len(all_rows) - 1
        }

        wb.close()
        return result


# 注册解析器
ParserFactory.register('excel', ExcelParser)
ParserFactory.register('xlsx', ExcelParser)
ParserFactory.register('xls', ExcelParser)
