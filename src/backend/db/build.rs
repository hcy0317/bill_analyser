use std::{
    env, fs,
    path::{Path, PathBuf},
};

const MIGRATIONS_RELATIVE_DIR: &str = "postgres/migrations";

fn main() {
    println!("cargo:rerun-if-changed={MIGRATIONS_RELATIVE_DIR}");

    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .expect("CARGO_MANIFEST_DIR is required to embed PostgreSQL migrations"),
    );
    let migrations_dir = manifest_dir.join(MIGRATIONS_RELATIVE_DIR);
    let migrations = discover_migrations(&migrations_dir);
    let generated = render_embedded_migrator(&migrations);
    let output = PathBuf::from(
        env::var_os("OUT_DIR").expect("OUT_DIR is required to embed PostgreSQL migrations"),
    )
    .join("embedded_postgres_migrations.rs");
    fs::write(&output, generated).unwrap_or_else(|error| {
        panic!(
            "failed to generate embedded PostgreSQL migrations at {}: {error}",
            output.display()
        )
    });
}

fn discover_migrations(directory: &Path) -> Vec<(i64, String, String)> {
    let entries = fs::read_dir(directory).unwrap_or_else(|error| {
        panic!(
            "PostgreSQL migration source directory is required at build time ({}): {error}",
            directory.display()
        )
    });
    let mut migrations = entries
        .filter_map(|entry| {
            let entry = entry.unwrap_or_else(|error| {
                panic!(
                    "failed to inspect PostgreSQL migration source directory {}: {error}",
                    directory.display()
                )
            });
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("sql") {
                return None;
            }
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_else(|| panic!("PostgreSQL migration filename must be UTF-8"))
                .to_string();
            let stem = file_name
                .strip_suffix(".sql")
                .expect("SQL migration suffix checked above");
            let (raw_version, raw_description) = stem.split_once('_').unwrap_or_else(|| {
                panic!(
                    "PostgreSQL migration filename must be <version>_<description>.sql: {file_name}"
                )
            });
            let version = raw_version.parse::<i64>().unwrap_or_else(|error| {
                panic!("invalid PostgreSQL migration version in {file_name}: {error}")
            });
            Some((version, raw_description.replace('_', " "), file_name))
        })
        .collect::<Vec<_>>();
    migrations.sort_by_key(|migration| migration.0);
    assert!(
        !migrations.is_empty(),
        "PostgreSQL migration source directory contains no SQL migrations: {}",
        directory.display()
    );
    for (index, (version, _, file_name)) in migrations.iter().enumerate() {
        let expected = i64::try_from(index + 1).expect("migration count fits i64");
        assert_eq!(
            *version, expected,
            "PostgreSQL migration versions must be unique and contiguous; expected {expected}, found {version} in {file_name}"
        );
    }
    migrations
}

fn render_embedded_migrator(migrations: &[(i64, String, String)]) -> String {
    let mut output = String::from(
        "fn embedded_postgres_migrator() -> sqlx::migrate::Migrator {\n    sqlx::migrate::Migrator {\n        migrations: std::borrow::Cow::Owned(vec![\n",
    );
    for (version, description, file_name) in migrations {
        output.push_str(&format!(
            "            sqlx::migrate::Migration::new({version}, std::borrow::Cow::Borrowed({description:?}), sqlx::migrate::MigrationType::Simple, std::borrow::Cow::Borrowed(include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{MIGRATIONS_RELATIVE_DIR}/{file_name}\"))), false),\n"
        ));
    }
    output.push_str("        ]),\n        ..sqlx::migrate::Migrator::DEFAULT\n    }\n}\n");
    output
}
