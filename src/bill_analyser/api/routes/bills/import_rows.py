# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403

def _read_text_with_fallback(file_path: Path, requested_encoding: str = "") -> tuple[str, str]:
    """按多种编码回退读取文本文件。"""
    encodings = []
    if requested_encoding:
        encodings.append(str(requested_encoding).strip())
    encodings.extend(["utf-8", "utf-8-sig", "gbk", "gb18030"])

    seen = set()
    for encoding in encodings:
        if not encoding or encoding.lower() in seen:
            continue
        seen.add(encoding.lower())
        try:
            with open(file_path, encoding=encoding) as file_obj:
                return file_obj.read(), encoding
        except UnicodeDecodeError:
            continue

    raise UnicodeDecodeError("unknown", b"", 0, 1, "unable to decode text file")


def _detect_csv_delimiter(sample_text: str, fallback: str = ",") -> str:
    """检测 CSV/TXT 分隔符。"""
    if not sample_text:
        return fallback

    candidates = [",", "\t", ";", "|"]

    try:
        detected_delimiter = csv.Sniffer().sniff(sample_text).delimiter
        if detected_delimiter in candidates and sample_text.count(detected_delimiter) > 0:
            return detected_delimiter
    except csv.Error:

        pass

    counts = {candidate: sample_text.count(candidate) for candidate in candidates}
    best = max(counts.items(), key=lambda item: item[1])
    return best[0] if best[1] > 0 else fallback


def _load_generic_import_rows(
    file_path: Path, requested_encoding: str = "", delimiter: str | None = None
) -> tuple[list[list[str]], str, str]:
    """读取通用表格文件为二维数组。"""
    suffix = file_path.suffix.lower()

    if suffix in (".csv", ".txt"):
        text, actual_encoding = _read_text_with_fallback(file_path, requested_encoding)
        actual_delimiter = delimiter or _detect_csv_delimiter(text[:2048], ",")
        reader = csv.reader(text.splitlines(), delimiter=actual_delimiter)
        rows = [[str(cell).strip() for cell in row] for row in reader]
        return rows, actual_encoding, actual_delimiter

    if suffix == ".xlsx":
        if openpyxl is None:
            raise ImportError("需要安装 openpyxl 以支持 Excel 导入")

        workbook = openpyxl.load_workbook(file_path, read_only=True, data_only=True)
        try:
            sheet = workbook.worksheets[0]
            rows = []
            for row in sheet.iter_rows(values_only=True):
                rows.append(["" if cell is None else str(cell).strip() for cell in row])
            return rows, "utf-8", ""
        finally:
            workbook.close()

    if suffix == ".xls":
        if pd is None:
            raise ImportError("需要安装 pandas 以支持旧版 xls 导入")

        html_error: Exception | None = None
        try:
            tables = pd.read_html(file_path)
            if not tables:
                raise ValueError("未在 xls 文件中找到可读取的表格")

            table = tables[0].fillna("")
            rows = []
            for record in table.itertuples(index=False, name=None):
                rows.append([str(cell).strip() for cell in record])
            return rows, "utf-8", ""
        except (ImportError, ValueError) as exc:
            html_error = exc

        try:
            table = pd.read_excel(file_path, header=None).fillna("")
            rows = []
            for record in table.itertuples(index=False, name=None):
                rows.append([str(cell).strip() for cell in record])
            return rows, "utf-8", ""
        except Exception:
            raise ValueError(f"未能读取 xls 文件: {file_path.name}") from html_error

    raise ValueError(f"Unsupported file format for generic import: {suffix}")


def _parse_generic_import_datetime(raw_value: Any, time_format: str = "") -> datetime | None:
    """严格解析导入时间，无法解析时返回 None，不把坏数据伪装成当前时间。"""
    parsed_datetime: datetime | None = None
    value = ""

    if isinstance(raw_value, datetime):
        parsed_datetime = raw_value.replace(microsecond=0)
    elif raw_value not in (None, ""):
        value = re.sub(r"\s+", " ", str(raw_value).replace("\t", " ")).strip()

    if parsed_datetime is None and value:
        candidate_formats = []
        if time_format:
            candidate_formats.append(time_format)
        candidate_formats.extend(
            [
                "%Y-%m-%d %H:%M:%S",
                "%Y/%m/%d %H:%M:%S",
                "%Y-%m-%d %H:%M",
                "%Y/%m/%d %H:%M",
                "%Y-%m-%d",
                "%Y/%m/%d",
                "%Y%m%d %H:%M:%S",
                "%Y%m%d %H:%M",
                "%Y%m%d",
                "%Y.%m.%d %H:%M:%S",
                "%Y.%m.%d",
                "%Y年%m月%d日 %H:%M:%S",
                "%Y年%m月%d日 %H:%M",
                "%Y年%m月%d日",
                "%d/%m/%Y %H:%M:%S",
                "%d/%m/%Y",
                "%m/%d/%Y %H:%M:%S",
                "%m/%d/%Y",
            ]
        )

        for fmt in candidate_formats:
            try:
                parsed_datetime = datetime.strptime(value, fmt).replace(microsecond=0)
                break
            except ValueError:
                continue

    if parsed_datetime is None and value:
        try:
            parsed_datetime = datetime.fromisoformat(value.replace("Z", "+00:00")).replace(
                microsecond=0
            )
        except ValueError:
            parsed = parse_bill_datetime(value)
            if parsed is not None:
                parsed_datetime = parsed.replace(microsecond=0)

    if parsed_datetime is None and value:
        logger.debug("[通用导入] 无法解析时间: %s", value)
    return parsed_datetime


def _parse_generic_import_time(raw_value: Any, time_format: str = "") -> int:
    """解析导入时间并返回 Unix 秒时间戳。

    兼容旧调用：旧导入预览工具函数在无法解析时仍返回当前时间戳；
    新的列映射入库路径必须使用 _parse_generic_import_datetime 做严格校验。
    """
    parsed_datetime = _parse_generic_import_datetime(raw_value, time_format)
    if parsed_datetime is None:
        return int(datetime.now().timestamp())
    return int(parsed_datetime.timestamp())


def _parse_generic_import_amount(
    raw_value: Any, decimal_separator: str = ".", grouping_symbol: str | None = None
) -> float:
    """解析导入金额。自动识别格式当未显式指定分隔符时。"""
    if raw_value in (None, ""):
        return 0.0

    if isinstance(raw_value, (int, float)):
        return abs(float(raw_value))

    value = str(raw_value).strip()
    if not value:
        return 0.0

    # 去除货币符号
    value = value.replace("¥", "").replace("￥", "").replace("$", "").replace("€", "").strip()

    # 处理括号表示负数
    if value.startswith("(") and value.endswith(")"):
        value = "-" + value[1:-1]

    # 自动检测小数/千位分隔符（仅当使用默认设定时）
    if decimal_separator == "." and not grouping_symbol:
        value = _auto_detect_and_normalize_amount(value)
    else:
        if grouping_symbol:
            value = value.replace(grouping_symbol, "")
        if decimal_separator and decimal_separator != ".":
            value = value.replace(decimal_separator, ".")
        # 移除可能残留的逗号（千位分隔符）
        value = value.replace(",", "")

    try:
        return abs(float(value))
    except ValueError:
        logger.debug("[通用导入] 无法解析金额，使用0: %s", raw_value)
        return 0.0


def _auto_detect_and_normalize_amount(value: str) -> str:
    """自动检测金额字符串的小数/千位分隔符格式并标准化为 Python float 可解析的格式。

    支持的格式：
    - 1234.56 / 1,234.56 / 1,234,567.89  (英式：逗号千位，点小数)
    - 1234,56 / 1.234,56 / 1.234.567,89  (欧式：点千位，逗号小数)
    - 1234 / 1,234 / 1.234               (纯整数或带千位)
    """
    # 去除空格
    cleaned = value.replace(" ", "").lstrip("+")

    # 提取符号
    sign = ""
    if cleaned.startswith("-"):
        sign = "-"
        cleaned = cleaned[1:]

    # 计数逗号和点
    comma_count = cleaned.count(",")
    dot_count = cleaned.count(".")

    if comma_count == 0 and dot_count == 0:
        # 纯数字
        return sign + cleaned
    elif comma_count == 0 and dot_count == 1:
        # "1234.56" 或 "1.234" — 通过小数部分长度判断
        parts = cleaned.split(".")
        if len(parts[1]) == 3 and len(parts[0]) <= 3:
            # 可能是千位分隔符（如 "1.234"），但也可能是正常小数（如 "0.123"）
            # 如果整数部分是 0 或有前导零，视为小数
            if parts[0] == "0" or (len(parts[0]) > 1 and parts[0].startswith("0")):
                return sign + cleaned  # 保持原样，"." 是小数点
            # 否则视为千位分隔符
            return sign + cleaned.replace(".", "")
        # 正常小数
        return sign + cleaned
    elif comma_count == 1 and dot_count == 0:
        # "1234,56" 或 "1,234" — 通过逗号后长度判断
        parts = cleaned.split(",")
        if len(parts[1]) == 3 and len(parts[0]) <= 3:
            # "1,234" — 千位分隔符
            return sign + cleaned.replace(",", "")
        # "1234,56" — 逗号是小数分隔符
        return sign + cleaned.replace(",", ".")
    elif dot_count >= 1 and comma_count == 1:
        # "1,234.56" 或 "1.234,56"
        last_comma = cleaned.rfind(",")
        last_dot = cleaned.rfind(".")
        if last_comma > last_dot:
            # "1.234,56" — 逗号是小数分隔符
            return sign + cleaned.replace(".", "").replace(",", ".")
        else:
            # "1,234.56" — 点是小数分隔符
            return sign + cleaned.replace(",", "")
    elif comma_count >= 2 and dot_count == 0:
        # "1,234,567" — 逗号是千位分隔符
        return sign + cleaned.replace(",", "")
    elif dot_count >= 2 and comma_count == 0:
        # "1.234.567" — 点是千位分隔符
        return sign + cleaned.replace(".", "")
    elif comma_count >= 2 and dot_count == 1:
        # "1,234,567.89" — 逗号千位，点小数
        return sign + cleaned.replace(",", "")
    else:
        # 无法确定，尝试去掉所有非数字/点字符
        return sign + cleaned.replace(",", "")


def _parse_generic_import_signed_amount(
    raw_value: Any, decimal_separator: str = ".", grouping_symbol: str | None = None
) -> float | None:
    """解析保留正负号的金额。"""
    if raw_value in (None, ""):
        return None

    if isinstance(raw_value, (int, float)):
        return float(raw_value)

    value = str(raw_value).strip()
    if not value:
        return None

    # 去除货币符号
    value = value.replace("¥", "").replace("￥", "").replace("$", "").replace("€", "").strip()

    # 处理括号表示负数
    if value.startswith("(") and value.endswith(")"):
        value = "-" + value[1:-1]

    if decimal_separator == "." and not grouping_symbol:
        value = _auto_detect_and_normalize_amount(value)
    else:
        if grouping_symbol:
            value = value.replace(grouping_symbol, "")
        if decimal_separator and decimal_separator != ".":
            value = value.replace(decimal_separator, ".")
        value = value.replace(",", "")

    try:
        return float(value)
    except ValueError:
        logger.debug("[通用导入] 无法解析带符号金额: %s", raw_value)
        return None

__all__ = [name for name in globals() if not name.startswith("__")]
