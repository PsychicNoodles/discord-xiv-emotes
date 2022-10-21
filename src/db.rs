pub mod models;

use log::*;
use sqlx::PgPool;
use thiserror::Error;

use self::models::{DbGender, DbLanguage, DbUser, DbUserOpt};

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
        discord_id: String,
        language: DbLanguage,
        gender: DbGender,
    ) -> Result<()> {
        debug!("upserting user {} {:?} {:?}", discord_id, language, gender);
        let user = DbUser {
            discord_id,
            language,
            gender,
            ..Default::default()
        };
        sqlx::query!(
            "
            INSERT INTO users (discord_id, language, gender, insert_tm, update_tm)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (discord_id) DO UPDATE
            SET discord_id = $1, language = $2, gender = $3, update_tm = $5
        ",
            user.discord_id,
            user.language as i32,
            user.gender as i32,
            user.insert_tm,
            user.update_tm
        )
        .execute(&self.0)
        .await?;
        Ok(())
    }

    pub async fn find_user(&self, discord_id: impl ToString) -> Result<DbUserOpt> {
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
        Ok(DbUserOpt(res))
    }
}
