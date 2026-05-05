"""System metadata routes."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/system/version", methods=["GET"])
@log_method
def get_system_version():
    """获取服务端版本信息。"""
    return jsonify({"success": True, "result": {"version": __version__, "commitHash": "", "buildTime": ""}})


__all__ = [name for name in globals() if not name.startswith("__")]
