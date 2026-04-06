"""农业银行 parser 识别回归。"""

from bill_analyser.parsers.abc import ABCParser
from bill_analyser.parsers.ccb import CCBParser
from bill_analyser.parsers.icbc import ICBCParser
from tests.parser_test_support import build_abc_xlsx


def test_abc_parser_distinguishes_abc_xlsx_from_other_bank_parsers(tmp_path) -> None:
    """农业银行样本应只命中 ABCParser，而不误伤工行/建行 parser。"""
    test_file = build_abc_xlsx(tmp_path / "abc_probe.xlsx")

    abc = ABCParser()
    icbc = ICBCParser()
    ccb = CCBParser()

    assert abc.can_parse(str(test_file)) is True
    assert icbc.can_parse(str(test_file)) is False
    assert ccb.can_parse(str(test_file)) is False
