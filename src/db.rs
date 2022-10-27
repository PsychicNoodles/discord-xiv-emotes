pub mod models;

use sqlx::PgPool;
use thiserror::Error;
use tracing::*;

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
    #[instrument]
    pub async fn upsert_user(
        &self,
        discord_id: impl AsRef<str> + std::fmt::Debug,
        language: DbLanguage,
        gender: DbGender,
    ) -> Result<()> {
        debug!("upserting user");
        let now = time::OffsetDateTime::now_utc();
        sqlx::query!(
            "
            INSERT INTO users (discord_id, language, gender, is_set_flg, insert_tm, update_tm)
            VALUES ($1, $2, $3, true, $4, $4)
            ON CONFLICT (discord_id) DO UPDATE
            SET discord_id = $1, language = $2, gender = $3, is_set_flg = true, update_tm = $4
        ",
            discord_id.as_ref(),
            language as i32,
            gender as i32,
            now
        )
        .execute(&self.0)
        .await?;
        Ok(())
    }

    #[instrument]
    pub async fn find_user(
        &self,
        discord_id: impl AsRef<str> + std::fmt::Debug,
    ) -> Result<Option<DbUser>> {
        debug!("finding user");
        let res = sqlx::query_as!(
            DbUser,
            r#"
            SELECT
                discord_id,
                language as "language: DbLanguage",
                gender as "gender: DbGender",
                is_set_flg,
                insert_tm,
                update_tm
            FROM users
            WHERE discord_id = $1
            "#,
            discord_id.as_ref()
        )
        .fetch_optional(&self.0)
        .await?;
        debug!("user lookup: {:?}", res);
        Ok(res)
    }

    #[instrument]
    pub async fn upsert_guild(
        &self,
        discord_id: impl AsRef<str> + std::fmt::Debug,
        language: DbLanguage,
        gender: DbGender,
        prefix: String,
    ) -> Result<()> {
        debug!("upserting guild");
        let now = time::OffsetDateTime::now_utc();
        sqlx::query!(
            "
            INSERT INTO guilds (discord_id, language, gender, prefix, is_set_flg, insert_tm, update_tm)
            VALUES ($1, $2, $3, $4, true, $5, $5)
            ON CONFLICT (discord_id) DO UPDATE
            SET discord_id = $1, language = $2, gender = $3, prefix = $4, is_set_flg = true, update_tm = $5
        ",
            discord_id.as_ref(),
            language as i32,
            gender as i32,
            prefix,
            now
        )
        .execute(&self.0)
        .await?;
        Ok(())
    }

    #[instrument]
    pub async fn find_guild(
        &self,
        discord_id: impl AsRef<str> + std::fmt::Debug,
    ) -> Result<Option<DbGuild>> {
        debug!("finding guild");
        let res = sqlx::query_as!(
            DbGuild,
            r#"
            SELECT
                discord_id,
                language as "language: DbLanguage",
                gender as "gender: DbGender",
                prefix,
                is_set_flg,
                insert_tm,
                update_tm
            FROM guilds
            WHERE discord_id = $1
            "#,
            discord_id.as_ref()
        )
        .fetch_optional(&self.0)
        .await?;
        debug!("guild lookup: {:?}", res.as_ref());
        Ok(res)
    }

    #[instrument]
    pub async fn insert_emote_log(
        &self,
        user_discord_id: impl AsRef<str> + std::fmt::Debug,
        guild_discord_id: Option<impl AsRef<str> + std::fmt::Debug>,
        emote_id: i32,
    ) -> Result<()> {
        debug!("inserting emote log");
        let now = time::OffsetDateTime::now_utc();
        let DbUser {
            language: user_language,
            gender: user_gender,
            ..
        } = DbUser::default();
        let user_id = sqlx::query!(
            "
            INSERT INTO users (discord_id, language, gender, is_set_flg, insert_tm, update_tm)
            VALUES ($1, $2, $3, false, $4, $4)
            ON CONFLICT (discord_id) DO UPDATE SET update_tm = $4
            RETURNING user_id
        ",
            user_discord_id.as_ref(),
            user_language as i32,
            user_gender as i32,
            now
        )
        .fetch_one(&self.0)
        .await?
        .user_id;

        let guild_id = if let Some(gdi) = guild_discord_id {
            let DbGuild {
                language: guild_language,
                gender: guild_gender,
                prefix: guild_prefix,
                ..
            } = DbGuild::default();
            Some(sqlx::query!(
                "
                INSERT INTO guilds (discord_id, language, gender, prefix, is_set_flg, insert_tm, update_tm)
                VALUES ($1, $2, $3, $4, false, $5, $5)
                ON CONFLICT (discord_id) DO UPDATE SET update_tm = $5
                RETURNING guild_id
            ",
                gdi.as_ref(),
                guild_language as i32,
                guild_gender as i32,
                guild_prefix,
                now
            )
            .fetch_one(&self.0)
            .await?
            .guild_id)
        } else {
            None
        };

        sqlx::query!(
            "
            INSERT INTO emote_logs (user_id, guild_id, emote_id, sent_at, insert_tm, update_tm)
            VALUES ($1, $2, $3, $4, $4, $4)
        ",
            user_id,
            guild_id,
            emote_id,
            now
        )
        .execute(&self.0)
        .await?;

        Ok(())
    }
}
