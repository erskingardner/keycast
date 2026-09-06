use keycast_core::v2::team_slug::{assign_slug, backfill_slugs, slugify};
use sqlx::{query::query, query_as::query_as, query_scalar::query_scalar, raw_sql::raw_sql};
use sqlx_sqlite::SqlitePoolOptions;

#[test]
fn readable_names_and_numeric_aliases_do_not_conflict() {
    assert_eq!(slugify("  My Personal TEAM! "), "my-personal-team");
    assert_eq!(slugify("2026"), "team-2026");
    assert_eq!(slugify("🔑 !!!"), "team");
    assert_eq!(slugify("Café 東京"), "café-東京");
    assert!(slugify(&"İ".repeat(120)).chars().count() <= 120);
}

#[tokio::test]
async fn populated_upgrade_is_unique_repeatable_and_preserves_renamed_team_links() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    raw_sql(include_str!("../../database/migrations/0001_initial.sql"))
        .execute(&pool)
        .await
        .unwrap();
    for name in [
        "Personal",
        "Personal",
        "Personal-2",
        "123",
        "!!!",
        "Café 東京",
    ] {
        query("INSERT INTO teams(name) VALUES (?)")
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }
    let before: Vec<(i64, String)> = query_as("SELECT id, name FROM teams ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    raw_sql(include_str!(
        "../../database/migrations/0002_team_slugs.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    backfill_slugs(&pool).await.unwrap();
    let after: Vec<(i64, String)> = query_as("SELECT id, name FROM teams ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    let slugs: Vec<String> = query_scalar("SELECT slug FROM teams ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(
        slugs,
        [
            "personal",
            "personal-2",
            "personal-2-2",
            "team-123",
            "team",
            "café-東京"
        ]
    );
    query("UPDATE teams SET name='Renamed' WHERE id=1")
        .execute(&pool)
        .await
        .unwrap();
    backfill_slugs(&pool).await.unwrap();
    assert_eq!(
        slugs,
        query_scalar::<_, String>("SELECT slug FROM teams ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap()
    );
    assert!(query("UPDATE teams SET slug='personal' WHERE id=2")
        .execute(&pool)
        .await
        .is_err());
    let mut transaction = pool.begin().await.unwrap();
    let id: i64 = query_scalar("INSERT INTO teams(name) VALUES('Personal') RETURNING id")
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
    assign_slug(&mut transaction, id, "Personal").await.unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(
        query_scalar::<_, String>("SELECT slug FROM teams WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "personal-3"
    );
}
