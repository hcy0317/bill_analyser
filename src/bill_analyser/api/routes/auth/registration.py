"""User registration routes and helpers."""

from __future__ import annotations

from .support import *  # noqa: F403


def validate_password(password: str, config: dict) -> tuple:
    """
    验证密码强度

    返回：
        tuple: (is_valid: bool, error_message: str)
    """
    min_length = config.get("password_min_length", 8)

    if len(password) < min_length:
        return False, f"Password must be at least {min_length} characters long"

    if config.get("password_require_uppercase", False):
        if not any(c.isupper() for c in password):
            return False, "Password must contain at least one uppercase letter"

    if config.get("password_require_lowercase", False):
        if not any(c.islower() for c in password):
            return False, "Password must contain at least one lowercase letter"

    if config.get("password_require_digit", False):
        if not any(c.isdigit() for c in password):
            return False, "Password must contain at least one digit"

    if config.get("password_require_special", False):
        special_chars = "!@#$%^&*()_+-=[]{}|;:,.<>?"
        if not any(c in special_chars for c in password):
            return False, "Password must contain at least one special character"

    return True, ""


def _build_default_accounts(language: str = "zh_Hans") -> list[dict[str, Any]]:
    """构建默认账户模板（国内常见账户）"""
    is_chinese = str(language).lower().startswith("zh")
    template_key = "zh" if is_chinese else "default"
    serialized_accounts: list[dict[str, Any]] = []

    for template in AUTH_DEFAULT_ACCOUNT_TEMPLATES[template_key]:
        account = dict(template)
        account["aliases"] = json.dumps(template["aliases"], ensure_ascii=not is_chinese)
        serialized_accounts.append(account)

    return serialized_accounts


async def _save_register_categories(db, user_id: int, categories: list[dict[str, Any]]) -> bool:
    """保存注册请求携带的预设分类"""
    if not categories:
        return True

    success = True

    for item in categories:
        try:
            main_category = str(item.get("name", "")).strip()
            if not main_category:
                continue

            category_type = int(item.get("type", 3))
            icon = str(item.get("icon", "") or "")
            color = str(item.get("color", "") or "")

            await db.create_category(
                {
                    "type": category_type,
                    "main_category": main_category,
                    "sub_category": "",
                    "description": "",
                    "priority": 0,
                    "keywords": "",
                    "hidden": False,
                    "icon": icon,
                    "color": color,
                },
                user_id=user_id,
            )

            sub_categories = item.get("subCategories", []) or []
            for sub_item in sub_categories:
                sub_category = str(sub_item.get("name", "")).strip()
                if not sub_category:
                    continue

                await db.create_category(
                    {
                        "type": category_type,
                        "main_category": main_category,
                        "sub_category": sub_category,
                        "description": "",
                        "priority": 0,
                        "keywords": "",
                        "hidden": False,
                        "icon": str(sub_item.get("icon", "") or icon),
                        "color": str(sub_item.get("color", "") or color),
                    },
                    user_id=user_id,
                )
        except Exception:  # pylint: disable=broad-except
            success = False
            logger.exception("保存注册预设分类失败: user_id=%s, category=%s", user_id, item)

    return success


async def _create_register_default_accounts(db, user_id: int, language: str) -> dict[str, Any]:
    """创建注册默认账户，并回填 default_account_id/cash_account_id"""
    result = {
        "success": True,
        "cash_account_id": None,
        "default_account_id": None,
    }

    try:
        default_accounts = _build_default_accounts(language)
        created_account_ids: list[int] = []

        for account_data in default_accounts:
            account_id = await db.create_account(account_data, user_id=user_id)
            created_account_ids.append(account_id)

            if account_data.get("category") == 1 and result["cash_account_id"] is None:
                result["cash_account_id"] = account_id

        if created_account_ids:
            if result["default_account_id"] is None:
                result["default_account_id"] = created_account_ids[0]

            await db.update_user(
                user_id,
                {
                    "default_account_id": result["default_account_id"],
                    "cash_account_id": result["cash_account_id"],
                },
            )
    except Exception:  # pylint: disable=broad-except
        result["success"] = False
        logger.exception("创建注册默认账户失败: user_id=%s", user_id)

    return result


@bp.route("/auth/register", methods=["POST"])
@log_method
def register():
    """
    用户注册端点

    请求体：
        {
            "username": "username",
            "email": "email@example.com",
            "password": "password",
            "nickname": "nickname",
            "language": "zh_Hans",
            "defaultCurrency": "CNY",
            "firstDayOfWeek": 1
        }
    """
    try:
        data = request.json or {}
        username = data.get("username", "").strip()
        email = data.get("email", "").strip()
        password = data.get("password", "")
        nickname = data.get("nickname", "").strip() or username
        register_categories = data.get("categories") if isinstance(data.get("categories"), list) else []

        # 验证必填字段
        if not username or not email or not password:
            logger.warning("注册失败: 缺少必填字段")
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "Username, email and password are required"}
            ), 400

        # 加载配置
        config = load_auth_config()

        # 检查是否允许注册
        if not config.get("enable_user_registration", True):
            logger.warning("注册失败: 注册功能已禁用")
            return jsonify(
                {
                    "success": False,
                    "error": "Registration disabled",
                    "message": "User registration is currently disabled",
                }
            ), 403

        # 验证密码强度
        logger.info("验证密码强度: username=%s", username)
        is_valid, error_msg = validate_password(password, config)
        if not is_valid:
            logger.warning("注册失败: 密码不符合要求 - %s", error_msg)
            return jsonify({"success": False, "error": "Invalid password", "message": error_msg}), 400

        db = get_app_context()
        ip_address = get_client_ip()
        user_agent = request.headers.get("User-Agent", "")

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 检查用户名是否已存在
        logger.info("检查用户名: username=%s", username)
        existing_user = loop.run_until_complete(db.get_user_by_username(username))
        if existing_user:
            logger.warning("注册失败: 用户名已存在 - %s", username)

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "username": username,
                        "event_type": "register_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "Username already exists",
                    }
                )
            )
            loop.close()

            return jsonify({"success": False, "error": "Username exists", "message": "Username already exists"}), 409

        # 检查邮箱是否已存在
        logger.info("检查邮箱: email=%s", email)
        existing_email = loop.run_until_complete(db.get_user_by_email(email))
        if existing_email:
            logger.warning("注册失败: 邮箱已存在 - %s", email)

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "username": username,
                        "event_type": "register_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "Email already exists",
                    }
                )
            )
            loop.close()

            return jsonify({"success": False, "error": "Email exists", "message": "Email already exists"}), 409

        # 哈希密码
        logger.info("正在哈希密码...")
        password_hash = bcrypt.hashpw(password.encode("utf-8"), bcrypt.gensalt()).decode("utf-8")

        # 创建用户
        logger.info("创建用户: username=%s, email=%s", username, email)
        user_id = loop.run_until_complete(
            db.create_user(
                {
                    "username": username,
                    "email": email,
                    "password_hash": password_hash,
                    "nickname": nickname,
                    "language": data.get("language", "zh_Hans"),
                    "default_currency": data.get("defaultCurrency", "CNY"),
                    "first_day_of_week": data.get("firstDayOfWeek", 1),
                    "is_active": 1,
                    "email_verified": 0 if config.get("require_email_verification") else 1,
                }
            )
        )

        normalized_register_categories = register_categories if isinstance(register_categories, list) else []
        preset_categories_saved = loop.run_until_complete(
            _save_register_categories(db, user_id, normalized_register_categories)
        )
        loop.run_until_complete(ensure_default_category_seed(db, user_id=user_id))

        default_accounts_result = loop.run_until_complete(
            _create_register_default_accounts(db, user_id, data.get("language", "zh_Hans"))
        )

        # 记录成功日志
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": username,
                    "event_type": "register_success",
                    "ip_address": ip_address,
                    "user_agent": user_agent,
                    "success": True,
                }
            )
        )

        loop.close()

        logger.info("注册成功: user_id=%s, username=%s, email=%s", user_id, username, email)

        return jsonify(
            {
                "success": True,
                "result": {
                    "user_id": user_id,
                    "username": username,
                    "email": email,
                    "needVerifyEmail": bool(config.get("require_email_verification")),
                    "presetCategoriesSaved": preset_categories_saved,
                    "presetAccountsSaved": bool(default_accounts_result.get("success")),
                    "message": "Registration successful",
                },
            }
        )

    except Exception as error:
        logger.error("注册过程发生错误: %s", error, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(error)}), 500


__all__ = [name for name in globals() if not name.startswith("__")]
