"""
Encryption Status Route - 数据库加密状态查询
"""

from flask import Blueprint, jsonify

from bill_analyser.core.db_encryption import get_encryption_config, is_sqlcipher_available

bp = Blueprint("encryption", __name__)


@bp.route("/status", methods=["GET"])
def encryption_status():
    """返回数据库加密状态。"""
    config = get_encryption_config()
    return jsonify({
        "success": True,
        "data": {
            "encrypted": config.enabled,
            "sqlcipher_available": is_sqlcipher_available(),
            "kdf_iter": config.kdf_iter,
            "cipher_page_size": config.cipher_page_size,
        },
    })
