"""Profile and avatar auth routes."""

from __future__ import annotations

from .support import *  # noqa: F403


def _build_user_profile_info(user: dict) -> dict:
    """构建统一的用户资料响应。"""
    username = user.get("username", "")
    email = user.get("email", "")
    nickname = user.get("nickname") or username
    investment_keyword_settings = build_user_investment_keyword_settings(user)
    default_account_id = user.get("default_account_id")
    cash_account_id = user.get("cash_account_id")
    cash_transfer_category_id = user.get("cash_transfer_category_id")

    return {
        "username": username,
        "email": email,
        "nickname": nickname,
        "avatar": user.get("avatar") or "",
        "avatarProvider": "internal",
        "defaultAccountId": str(default_account_id) if default_account_id not in (None, "") else "",
        "transactionEditScope": user.get("transaction_edit_scope", 0),
        "language": user.get("language") or "zh_Hans",
        "defaultCurrency": user.get("default_currency") or "CNY",
        "firstDayOfWeek": user.get("first_day_of_week", 1),
        "fiscalYearStart": user.get("fiscal_year_start", 1),
        "calendarDisplayType": user.get("calendar_display_type", 0),
        "dateDisplayType": user.get("date_display_type", 0),
        "longDateFormat": user.get("long_date_format", 0),
        "shortDateFormat": user.get("short_date_format", 0),
        "longTimeFormat": user.get("long_time_format", 0),
        "shortTimeFormat": user.get("short_time_format", 0),
        "fiscalYearFormat": user.get("fiscal_year_format", 0),
        "currencyDisplayType": user.get("currency_display_type", 0),
        "numeralSystem": user.get("numeral_system", 0),
        "decimalSeparator": user.get("decimal_separator", 0),
        "digitGroupingSymbol": user.get("digit_grouping_symbol", 0),
        "digitGrouping": user.get("digit_grouping", 0),
        "coordinateDisplayType": user.get("coordinate_display_type", 0),
        "expenseAmountColor": user.get("expense_amount_color", 0),
        "incomeAmountColor": user.get("income_amount_color", 0),
        "cashAccountId": str(cash_account_id) if cash_account_id not in (None, "") else "",
        "cashTransferCategoryId": str(cash_transfer_category_id) if cash_transfer_category_id not in (None, "") else "",
        "importLearningEnabled": bool(user.get("import_learning_enabled", True)),
        "investmentPlatformKeywords": investment_keyword_settings["platform_keywords"],
        "investmentProductKeywords": investment_keyword_settings["product_keywords"],
        "investmentExcludeKeywords": investment_keyword_settings["exclude_keywords"],
        "emailVerified": bool(user.get("email_verified", False)),
    }


def _build_avatar_data_url(uploaded_file) -> str:
    """将上传头像文件转为 data URL。"""
    content = uploaded_file.read()
    if not content:
        raise ValueError("Avatar file is empty")

    mime_type = uploaded_file.mimetype or mimetypes.guess_type(uploaded_file.filename or "")[0]
    if not mime_type:
        mime_type = "application/octet-stream"

    encoded = base64.b64encode(content).decode("utf-8")
    return f"data:{mime_type};base64,{encoded}"


@bp.route("/profile", methods=["GET", "PUT"])
@log_method
@require_auth
def profile():
    """
    获取或更新用户资料端点（需要认证）

    GET: 获取用户资料
    PUT: 更新用户资料
    """
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        if request.method == "GET":
            # 获取用户资料
            user = loop.run_until_complete(db.get_user_by_id(user_id))
            loop.close()

            if not user:
                return jsonify({"success": False, "error": "User not found"}), 404

            user_info = _build_user_profile_info(user)

            logger.info(
                "返回用户资料: user_id=%s, username=%s, calendarDisplayType=%s, fiscalYearStart=%s",
                user_id,
                user_info["username"],
                user_info["calendarDisplayType"],
                user_info["fiscalYearStart"],
            )
            logger.debug("完整用户资料数据: %s", user_info)

            return jsonify({"success": True, "result": user_info})

        # PUT - 更新用户资料
        data = request.json
        if not data:
            return jsonify({"success": False, "error": "Bad Request", "message": "Request body is required"}), 400

        update_data = {}

        # 允许更新的字段（基本信息）
        if "nickname" in data:
            update_data["nickname"] = data["nickname"]
        if "email" in data:
            update_data["email"] = data["email"]
        if "avatar" in data:
            update_data["avatar"] = data["avatar"]
        if "language" in data:
            update_data["language"] = data["language"]
        if "defaultCurrency" in data:
            update_data["default_currency"] = data["defaultCurrency"]
        if "firstDayOfWeek" in data:
            update_data["first_day_of_week"] = data["firstDayOfWeek"]
        if "defaultAccountId" in data:
            update_data["default_account_id"] = data["defaultAccountId"]
        if "transactionEditScope" in data:
            update_data["transaction_edit_scope"] = data["transactionEditScope"]

        # 允许更新的字段（显示配置）
        if "fiscalYearStart" in data:
            update_data["fiscal_year_start"] = data["fiscalYearStart"]
        if "calendarDisplayType" in data:
            update_data["calendar_display_type"] = data["calendarDisplayType"]
        if "dateDisplayType" in data:
            update_data["date_display_type"] = data["dateDisplayType"]
        if "longDateFormat" in data:
            update_data["long_date_format"] = data["longDateFormat"]
        if "shortDateFormat" in data:
            update_data["short_date_format"] = data["shortDateFormat"]
        if "longTimeFormat" in data:
            update_data["long_time_format"] = data["longTimeFormat"]
        if "shortTimeFormat" in data:
            update_data["short_time_format"] = data["shortTimeFormat"]
        if "fiscalYearFormat" in data:
            update_data["fiscal_year_format"] = data["fiscalYearFormat"]
        if "currencyDisplayType" in data:
            update_data["currency_display_type"] = data["currencyDisplayType"]
        if "numeralSystem" in data:
            update_data["numeral_system"] = data["numeralSystem"]
        if "decimalSeparator" in data:
            update_data["decimal_separator"] = data["decimalSeparator"]
        if "digitGroupingSymbol" in data:
            update_data["digit_grouping_symbol"] = data["digitGroupingSymbol"]
        if "digitGrouping" in data:
            update_data["digit_grouping"] = data["digitGrouping"]
        if "coordinateDisplayType" in data:
            update_data["coordinate_display_type"] = data["coordinateDisplayType"]
        if "expenseAmountColor" in data:
            update_data["expense_amount_color"] = data["expenseAmountColor"]
        if "incomeAmountColor" in data:
            update_data["income_amount_color"] = data["incomeAmountColor"]
        if "cashAccountId" in data:
            update_data["cash_account_id"] = data["cashAccountId"]
        if "cashTransferCategoryId" in data:
            update_data["cash_transfer_category_id"] = data["cashTransferCategoryId"]
        if "importLearningEnabled" in data:
            update_data["import_learning_enabled"] = 1 if data["importLearningEnabled"] else 0
        if "investmentPlatformKeywords" in data:
            update_data["investment_platform_keywords"] = serialize_keyword_list(data["investmentPlatformKeywords"])
        if "investmentProductKeywords" in data:
            update_data["investment_product_keywords"] = serialize_keyword_list(data["investmentProductKeywords"])
        if "investmentExcludeKeywords" in data:
            update_data["investment_exclude_keywords"] = serialize_keyword_list(data["investmentExcludeKeywords"])

        logger.info("将更新用户资料: user_id=%s, fields=%s", user_id, list(update_data.keys()))
        logger.debug("更新数据详情: %s", update_data)

        if update_data:
            success = loop.run_until_complete(db.update_user(user_id, update_data))
            if not success:
                loop.close()
                return jsonify(
                    {"success": False, "error": "Update failed", "message": "Failed to update user profile"}
                ), 500
            logger.info("用户资料更新成功: user_id=%s", user_id)

        # 返回更新后的用户信息
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        loop.close()

        if not user:
            return jsonify({"success": False, "error": "User not found after update"}), 404

        user_info = _build_user_profile_info(user)

        logger.info("用户资料更新并返回: user_id=%s, updated_fields=%s", user_id, len(update_data))
        logger.debug("返回的用户资料: %s", user_info)

        # v6.79: 前端 updateUserProfile() 期望 result.user 结构
        # 见 index.ts 第 606 行：if (data.result.user && isObject(data.result.user))
        return jsonify({"success": True, "result": {"user": user_info}})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("处理用户资料失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/profile/avatar", methods=["POST"])
@log_method
@require_auth
def update_profile_avatar():
    """更新当前用户头像（REST）。"""
    loop = None
    try:
        avatar_file = request.files.get("avatar")
        if not avatar_file:
            return jsonify({"success": False, "error": "Bad Request", "message": "Avatar file is required"}), 400

        avatar_data_url = _build_avatar_data_url(avatar_file)
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        success = loop.run_until_complete(db.update_user(user_id, {"avatar": avatar_data_url}))
        if not success:
            loop.close()
            return jsonify({"success": False, "error": "Update failed", "message": "Failed to update avatar"}), 500

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        loop.close()
        if not user:
            return jsonify({"success": False, "error": "User not found"}), 404

        return jsonify({"success": True, "result": _build_user_profile_info(user)})
    except ValueError as exc:
        if loop and not loop.is_closed():
            loop.close()
        return jsonify({"success": False, "error": "Bad Request", "message": str(exc)}), 400
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("更新用户头像失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/profile/avatar", methods=["DELETE"])
@log_method
@require_auth
def remove_profile_avatar():
    """删除当前用户头像（REST）。"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        success = loop.run_until_complete(db.update_user(user_id, {"avatar": ""}))
        if not success:
            loop.close()
            return jsonify({"success": False, "error": "Update failed", "message": "Failed to remove avatar"}), 500

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        loop.close()
        if not user:
            return jsonify({"success": False, "error": "User not found"}), 404

        return jsonify({"success": True, "result": _build_user_profile_info(user)})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("删除用户头像失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/profile/email/resend-verification", methods=["POST"])
@log_method
@require_auth
def resend_profile_verification_email():
    """重发当前用户的验证邮件（REST）。

    当前项目尚未接入真实邮件发送基础设施，因此此端点提供
    与前端契约一致的幂等成功响应，并记录审计日志，便于后续
    接入 SMTP/第三方邮件服务时平滑扩展。
    """
    loop = None
    try:
        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        username = _get_request_username()

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        email = (user.get("email") or "").strip()
        if not email:
            loop.close()
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Email is required to resend verification email"}
            ), 400

        email_verified = bool(user.get("email_verified", False))
        require_email_verification = bool(config.get("require_email_verification", False))

        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": username,
                    "event_type": "verification_email_resend_requested",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                    "metadata": json.dumps(
                        {
                            "email": email,
                            "email_verified": email_verified,
                            "require_email_verification": require_email_verification,
                            "delivery": "not_configured_mock_success",
                        },
                        ensure_ascii=False,
                    ),
                }
            )
        )
        loop.close()

        logger.info(
            "重发验证邮件请求已记录: user_id=%s, email=%s, verified=%s, require_email_verification=%s",
            user_id,
            email,
            email_verified,
            require_email_verification,
        )

        return jsonify({"success": True, "result": True})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("重发验证邮件失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


__all__ = [name for name in globals() if not name.startswith("__")]
