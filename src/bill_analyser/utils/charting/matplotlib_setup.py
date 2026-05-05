"""Shared matplotlib setup for chart generation."""

import matplotlib
import matplotlib.dates as mdates
import matplotlib.pyplot as plt

# 使用非交互式后端
matplotlib.use("Agg")

# 设置中文字体
plt.rcParams["font.sans-serif"] = ["SimHei", "Microsoft YaHei", "Arial Unicode MS"]
plt.rcParams["axes.unicode_minus"] = False  # 解决负号显示问题

__all__ = ["matplotlib", "mdates", "plt"]
