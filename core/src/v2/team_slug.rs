use sqlx::{query::query, query_as::query_as, query_scalar::query_scalar};
use sqlx_sqlite::SqliteConnection;

pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.to_lowercase().chars().take(120) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "team".to_owned()
    } else if slug.chars().all(|c| c.is_ascii_digit()) {
        format!("team-{slug}")
    } else {
        slug.to_owned()
    }
}

/// Caller must hold a write transaction. The unique index is the final collision guard.
pub async fn assign_slug(
    connection: &mut SqliteConnection,
    id: i64,
    name: &str,
) -> Result<(), sqlx::Error> {
    let base = slugify(name);
    let mut candidate = base.clone();
    let mut suffix = 2;
    while query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM teams WHERE slug = ?)")
        .bind(&candidate)
        .fetch_one(&mut *connection)
        .await?
    {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    query("UPDATE teams SET slug = ? WHERE id = ? AND slug IS NULL")
        .bind(candidate)
        .bind(id)
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn backfill_slugs(pool: &sqlx_sqlite::SqlitePool) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    // Acquire the writer before reading and assigning names, including during local upgrades.
    query("UPDATE teams SET slug = slug WHERE slug IS NULL")
        .execute(&mut *transaction)
        .await?;
    let teams: Vec<(i64, String)> =
        query_as("SELECT id, name FROM teams WHERE slug IS NULL ORDER BY id")
            .fetch_all(&mut *transaction)
            .await?;
    for (id, name) in teams {
        assign_slug(&mut transaction, id, &name).await?;
    }
    transaction.commit().await
}
