// 中文导读：分类路径兼容投影层，统一解释 PostgreSQL categories.path 与旧 name fallback。
// 维护重点：这里只处理存储路径到主/子分类展示名的纯映射，不吸收 payload 优先级或规则选择。

use sqlx::{Postgres, QueryBuilder};

const POSTGRES_RUST_WHITESPACE_CHARS_SQL: &str = r#"U&' \0009\000A\000B\000C\000D\0085\00A0\1680\2000\2001\2002\2003\2004\2005\2006\2007\2008\2009\200A\2028\2029\202F\205F\3000'"#;
const POSTGRES_RUST_EXTRA_WHITESPACE_SQL: &str = r#"U&'\0085\00A0\1680\2000\2001\2002\2003\2004\2005\2006\2007\2008\2009\200A\2028\2029\202F\205F\3000'"#;

pub(crate) fn category_names_from_postgres_path(
    path: Option<&str>,
    fallback_name: &str,
) -> (String, String) {
    let parts = path
        .unwrap_or_default()
        .split('/')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => (fallback_name.to_string(), String::new()),
        [main] => ((*main).to_string(), String::new()),
        [main, rest @ ..] => ((*main).to_string(), rest.join("/")),
    }
}

pub(crate) fn push_postgres_bill_main_category_expr(builder: &mut QueryBuilder<'_, Postgres>) {
    builder.push("COALESCE(NULLIF(b.standard_payload->>'main_category', ''), NULLIF(split_part(");
    push_normalized_postgres_category_path_expr(builder);
    builder.push(", '/', 1), ''), c.name, '')");
}

pub(crate) fn push_postgres_bill_sub_category_expr(builder: &mut QueryBuilder<'_, Postgres>) {
    builder.push("COALESCE(NULLIF(b.standard_payload->>'sub_category', ''), regexp_replace(");
    push_normalized_postgres_category_path_expr(builder);
    builder.push(", '^[^/]*/?', ''), '')");
}

fn push_normalized_postgres_category_path_expr(builder: &mut QueryBuilder<'_, Postgres>) {
    builder.push("btrim(regexp_replace(regexp_replace(btrim(COALESCE(c.path, ''), ");
    builder.push(POSTGRES_RUST_WHITESPACE_CHARS_SQL);
    builder.push("), ('[[:space:]' || ");
    builder.push(POSTGRES_RUST_EXTRA_WHITESPACE_SQL);
    builder.push(" || ']*/[[:space:]' || ");
    builder.push(POSTGRES_RUST_EXTRA_WHITESPACE_SQL);
    builder.push(" || ']*'), '/', 'g'), '/+', '/', 'g'), '/')");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_category_path_projects_current_main_sub_and_fallback_contract() {
        for (path, fallback_name, expected) in [
            (None, "未分类", ("未分类", "")),
            (Some("餐饮"), "忽略", ("餐饮", "")),
            (Some("餐饮/午餐"), "忽略", ("餐饮", "午餐")),
            (
                Some("\t\u{a0}餐饮\u{a0} //\n 工作日 / 午餐\u{3000}\t"),
                "忽略",
                ("餐饮", "工作日/午餐"),
            ),
            (Some(" / / "), "兼容名称", ("兼容名称", "")),
        ] {
            let actual = category_names_from_postgres_path(path, fallback_name);
            assert_eq!(actual, (expected.0.to_string(), expected.1.to_string()));
        }
    }
}
