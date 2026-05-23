// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

use std::io::{self, Read};

use bill_analyser_core::auth::validate_refresh_token_claims;
use serde_json::{json, Value};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let command = std::env::args()
        .nth(1)
        .ok_or_else(|| "missing auth bridge command".to_string())?;

    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|_| "failed to read bridge input".to_string())?;

    let payload: Value =
        serde_json::from_str(&input).map_err(|_| "failed to parse bridge input".to_string())?;

    match command.as_str() {
        "validate-refresh-token-claims" => {
            let response = match validate_refresh_token_claims(&payload) {
                Ok(claims) => json!({
                    "success": true,
                    "result": {
                        "user_id": claims.user_id.get(),
                        "username": claims.username,
                    },
                }),
                Err(error) => json!({
                    "success": false,
                    "error": {
                        "status": error.status,
                        "error": error.error,
                        "message": error.message,
                    },
                }),
            };
            println!("{response}");
            Ok(())
        }
        _ => Err("unknown auth bridge command".to_string()),
    }
}
