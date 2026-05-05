"""Shared chart colors."""

# 颜色方案
COLORS = {
    "income": "#4CAF50",  # 绿色 - 收入
    "expense": "#F44336",  # 红色 - 支出
    "net": "#2196F3",  # 蓝色 - 净收入
    "primary": "#1976D2",  # 主色
    "secondary": "#FF9800",  # 辅色
    "background": "#FFFFFF",  # 背景色
    "grid": "#E0E0E0",  # 网格线
    "text": "#333333",  # 文字颜色
}

# 分类配色（循环使用）
CATEGORY_COLORS = [
    "#FF6B6B",
    "#4ECDC4",
    "#45B7D1",
    "#FFA07A",
    "#98D8C8",
    "#F7DC6F",
    "#BB8FCE",
    "#85C1E2",
    "#F8B195",
    "#C06C84",
    "#6C5B7B",
    "#355C7D",
    "#F67280",
    "#C06C84",
    "#6C5B7B",
]

__all__ = ["CATEGORY_COLORS", "COLORS"]
