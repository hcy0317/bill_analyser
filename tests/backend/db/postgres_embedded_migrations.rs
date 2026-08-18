use bill_analyser_db::{embedded_postgres_migration_versions, postgres_migrations_dir};

#[test]
fn embedded_migrations_cover_every_declared_postgres_migration() {
    let embedded_versions = embedded_postgres_migration_versions();
    let mut declared_versions = std::fs::read_dir(postgres_migrations_dir())
        .expect("migration directory")
        .map(|entry| {
            let file_name = entry
                .expect("migration entry")
                .file_name()
                .into_string()
                .expect("UTF-8 migration filename");
            file_name
                .split_once('_')
                .expect("versioned migration filename")
                .0
                .parse::<i64>()
                .expect("numeric migration version")
        })
        .collect::<Vec<_>>();
    declared_versions.sort_unstable();

    assert_eq!(embedded_versions, declared_versions);
    assert_eq!(embedded_versions.first(), Some(&1));
    assert_eq!(embedded_versions.last(), Some(&27));
}
