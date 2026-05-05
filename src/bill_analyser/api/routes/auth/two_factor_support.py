"""Two-factor authentication helper functions."""

from __future__ import annotations

from .support import *  # noqa: F403


def _set_recovery_codes(user_id: int, recovery_codes: list[str]) -> None:
    """缓存用户当前有效的 2FA 恢复码。"""
    TWO_FACTOR_RECOVERY_CODES[user_id] = [str(code).upper() for code in recovery_codes]


def _consume_recovery_code(user_id: int, recovery_code: str) -> bool:
    """消费单个恢复码。"""
    normalized = str(recovery_code or "").strip().upper()
    current_codes = TWO_FACTOR_RECOVERY_CODES.get(user_id, [])

    if normalized not in current_codes:
        return False

    current_codes.remove(normalized)
    TWO_FACTOR_RECOVERY_CODES[user_id] = current_codes
    return True


def _generate_2fa_qrcode_data_url(username: str, secret: str) -> str:
    """生成 2FA 二维码 data URL。"""
    provisioning_uri = pyotp.TOTP(secret).provisioning_uri(name=username, issuer_name="Bill Analyser")

    qr_image = qrcode.make(provisioning_uri)
    buffer = io.BytesIO()
    qr_image.save(buffer, "PNG")
    encoded = base64.b64encode(buffer.getvalue()).decode("utf-8")
    return f"data:image/png;base64,{encoded}"


def _generate_recovery_codes() -> list[str]:
    """生成恢复码列表。"""
    return [f"{secrets.token_hex(4)[:4]}-{secrets.token_hex(4)[:4]}".upper() for _ in range(8)]


def _replace_persistent_recovery_codes(db, user_id: int, recovery_codes: list[str], loop) -> int:
    """优先使用数据库持久化恢复码；仅在测试桩缺失实现时回退到内存缓存。"""
    replace_method = getattr(db, "replace_two_factor_recovery_codes", None)
    if callable(replace_method):
        stored_count = loop.run_until_complete(replace_method(user_id, recovery_codes))
        TWO_FACTOR_RECOVERY_CODES.pop(user_id, None)
        return int(stored_count or 0)

    _set_recovery_codes(user_id, recovery_codes)
    return len(recovery_codes)


def _clear_persistent_recovery_codes(db, user_id: int, loop) -> int:
    """清空持久化恢复码，并同步清理测试兼容用的内存缓存。"""
    clear_method = getattr(db, "clear_two_factor_recovery_codes", None)
    cleared_count = 0
    if callable(clear_method):
        cleared_count = int(loop.run_until_complete(clear_method(user_id)) or 0)

    TWO_FACTOR_RECOVERY_CODES.pop(user_id, None)
    return cleared_count


def _consume_persistent_recovery_code(db, user_id: int, recovery_code: str, loop) -> bool:
    """消费恢复码，数据库持久化优先，测试兼容场景回退到内存 helper。"""
    consume_method = getattr(db, "consume_two_factor_recovery_code", None)
    if callable(consume_method) and loop.run_until_complete(consume_method(user_id, recovery_code)):
        return True

    return _consume_recovery_code(user_id, recovery_code)


def _create_two_factor_audit_log(
    db,
    loop,
    *,
    operation_type: str,
    user_id: int,
    details: dict[str, Any] | None = None,
    affected_count: int = 0,
) -> None:
    """为 2FA 关键动作写审计日志；失败时只告警，不影响主流程。"""
    create_audit_log = getattr(db, "create_audit_log", None)
    if not callable(create_audit_log):
        return

    try:
        loop.run_until_complete(
            create_audit_log(
                operation_type=operation_type,
                operation_target="user",
                target_id=user_id,
                details=details,
                affected_count=affected_count,
                ip_address=get_client_ip(),
                user_agent=request.headers.get("User-Agent", ""),
                session_id=str(getattr(request, "session_id", "") or "") or None,
                status="success",
            )
        )
    except Exception as exc:  # pylint: disable=broad-except
        logger.warning("记录 2FA 审计日志失败: operation=%s, user_id=%s, error=%s", operation_type, user_id, exc)


__all__ = [name for name in globals() if not name.startswith("__")]
