use sqlx::PgPool;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database error ({0})")]
    Db(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(sqlx::Type)]
#[repr(i32)]
pub enum UserLanguage {
    En = 0,
    Ja = 1,
}

#[derive(sqlx::Type)]
#[repr(i32)]
pub enum UserGender {
    M = 0,
    F = 1,
}

#[derive(sqlx::Type)]
#[sqlx(type_name = "user")]
pub struct User {
    pub discord_id: String,
    pub language: UserLanguage,
    pub gender: UserGender,
    pub insert_tm: time::PrimitiveDateTime,
    pub update_tm: time::PrimitiveDateTime,
}

pub async fn insert_user(
    pool: &PgPool,
    discord_id: String,
    language: UserLanguage,
    gender: UserGender,
) -> Result<()> {
    sqlx::query(
        "
            INSERT INTO users (discord_id, language, gender)
            VALUES (?, ?, ?)
        ",
    )
    .bind(discord_id)
    .bind(language)
    .bind(gender)
    .execute(pool)
    .await?;
    Ok(())
}
