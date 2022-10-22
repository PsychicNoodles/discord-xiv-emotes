pub mod models;

use log::*;
use sqlx::PgPool;
use thiserror::Error;

use self::models::{DbGender, DbGuild, DbLanguage, DbUser};

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database error ({0})")]
    Db(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(Debug)]
pub struct Db(pub PgPool);

impl Db {
    pub async fn upsert_user(
        &self,
        discord_id: impl ToString,
        language: DbLanguage,
        gender: DbGender,
    ) -> Result<()> {
        let discord_id = discord_id.to_string();
        debug!("upserting user {} {:?} {:?}", discord_id, language, gender);
        let now = time::OffsetDateTime::now_utc();
        sqlx::query!(
            "
            INSERT INTO users (discord_id, language, gender, insert_tm, update_tm)
            VALUES ($1, $2, $3, $4, $4)
            ON CONFLICT (discord_id) DO UPDATE
            SET discord_id = $1, language = $2, gender = $3, update_tm = $4
        ",
            discord_id,
            language as i32,
            gender as i32,
            now
        )
        .execute(&self.0)
        .await?;
        Ok(())
    }

    pub async fn find_user(&self, discord_id: impl ToString) -> Result<Option<DbUser>> {
        let discord_id = discord_id.to_string();
        debug!("checking for user {:?}", discord_id);
        let res = sqlx::query_as!(
            DbUser,
            r#"
            SELECT
                discord_id,
                language as "language: DbLanguage",
                gender as "gender: DbGender",
                insert_tm,
                update_tm
            FROM users
            WHERE discord_id = $1
            "#,
            discord_id
        )
        .fetch_optional(&self.0)
        .await?;
        debug!("user lookup: {:?}", res);
        Ok(res)
    }

    pub async fn upsert_guild(
        &self,
        discord_id: impl ToString,
        language: DbLanguage,
        gender: DbGender,
    ) -> Result<()> {
        let discord_id = discord_id.to_string();
        debug!("upserting guild {} {:?} {:?}", discord_id, language, gender);
        let now = time::OffsetDateTime::now_utc();
        sqlx::query!(
            "
            INSERT INTO guilds (discord_id, language, gender, insert_tm, update_tm)
            VALUES ($1, $2, $3, $4, $4)
            ON CONFLICT (discord_id) DO UPDATE
            SET discord_id = $1, language = $2, gender = $3, update_tm = $4
        ",
            discord_id,
            language as i32,
            gender as i32,
            now
        )
        .execute(&self.0)
        .await?;
        Ok(())
    }

    pub async fn find_guild(&self, discord_id: impl ToString) -> Result<Option<DbGuild>> {
        let discord_id = discord_id.to_string();
        debug!("checking for guild {:?}", discord_id);
        let res = sqlx::query_as!(
            DbGuild,
            r#"
            SELECT
                discord_id,
                language as "language: DbLanguage",
                gender as "gender: DbGender",
                insert_tm,
                update_tm
            FROM guilds
            WHERE discord_id = $1
            "#,
            discord_id
        )
        .fetch_optional(&self.0)
        .await?;
        debug!("guild lookup: {:?}", res);
        Ok(res)
    }
}
