// 中文导读：导入运行时共享的账户与分类目录读取，向 Stage 2 和 LLM adapter 提供同一份 typed records。
// 维护重点：本模块只拥有 active、user-scoped SQL 与 row mapping；prompt/分类/账户识别语义留在调用方。
// 不变式：返回顺序固定为 display_order、id；每次调用读取当前权威数据，不跨请求缓存。

use sqlx::Row;

use crate::{DbResult, PostgresPool};

#[derive(Debug, Clone)]
pub struct ImportCategoryCatalogRecord {
    pub id: i64,
    pub category_type: Option<String>,
    pub path: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ImportAccountCatalogRecord {
    pub id: i64,
    pub name: String,
}

pub async fn load_import_category_catalog_records(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<ImportCategoryCatalogRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, category_type, path, name
        FROM categories
        WHERE user_id = $1 AND is_active = true
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ImportCategoryCatalogRecord {
                id: row.try_get("id")?,
                category_type: row.try_get("category_type")?,
                path: row.try_get("path")?,
                name: row.try_get("name")?,
            })
        })
        .collect()
}

pub async fn load_import_account_catalog_records(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<ImportAccountCatalogRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name
        FROM accounts
        WHERE user_id = $1 AND is_active = true
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ImportAccountCatalogRecord {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
            })
        })
        .collect()
}
