pub mod models;
pub mod util;

use std::borrow::Borrow;

use futures::{stream, StreamExt, TryStreamExt};
use serenity::model::prelude::{GuildId, UserId};
use sqlx::{PgPool, QueryBuilder};
use tracing::*;

use crate::{commands::stats::EmoteLogQuery, HandlerError};

use self::models::{DbGender, DbGuild, DbLanguage, DbUser};
use self::util::DiscordIdExt;

#[derive(Debug)]
pub struct Db(pub PgPool);

impl Db {
    #[instrument]
    pub async fn upsert_user(
        &self,
        discord_id: &UserId,
        language: DbLanguage,
        gender: DbGender,
    ) -> Result<(), HandlerError> {
        debug!("upserting user");
        let now = time::OffsetDateTime::now_utc();
        sqlx::query!(
            "
            INSERT INTO users (discord_id, language, gender, is_set_flg, insert_tm, update_tm)
            VALUES ($1, $2, $3, true, $4, $4)
            ON CONFLICT (discord_id) DO UPDATE
            SET discord_id = $1, language = $2, gender = $3, is_set_flg = true, update_tm = $4
            ",
            discord_id.to_db_string(),
            language as i32,
            gender as i32,
            now
        )
        .execute(&self.0)
        .await?;
        Ok(())
    }

    #[instrument]
    pub async fn find_user(&self, discord_id: &UserId) -> Result<Option<DbUser>, HandlerError> {
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
            discord_id.to_db_string()
        )
        .fetch_optional(&self.0)
        .await?;
        debug!("user lookup: {:?}", res);
        Ok(res)
    }

    #[instrument]
    pub async fn upsert_guild(
        &self,
        discord_id: &GuildId,
        language: DbLanguage,
        gender: DbGender,
        prefix: String,
    ) -> Result<(), HandlerError> {
        debug!("upserting guild");
        let now = time::OffsetDateTime::now_utc();
        sqlx::query!(
            "
            INSERT INTO guilds (discord_id, language, gender, prefix, is_set_flg, insert_tm, update_tm)
            VALUES ($1, $2, $3, $4, true, $5, $5)
            ON CONFLICT (discord_id) DO UPDATE
            SET discord_id = $1, language = $2, gender = $3, prefix = $4, is_set_flg = true, update_tm = $5
            ",
            discord_id.to_db_string(),
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
    pub async fn find_guild(&self, discord_id: &GuildId) -> Result<Option<DbGuild>, HandlerError> {
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
            discord_id.to_db_string()
        )
        .fetch_optional(&self.0)
        .await?;
        debug!("guild lookup: {:?}", res.as_ref());
        Ok(res)
    }

    async fn upsert_user_not_set(
        &self,
        user_discord_id: &UserId,
        user_language: DbLanguage,
        user_gender: DbGender,
        now: time::OffsetDateTime,
    ) -> Result<i64, HandlerError> {
        Ok(sqlx::query!(
            "
            INSERT INTO users (discord_id, language, gender, is_set_flg, insert_tm, update_tm)
            VALUES ($1, $2, $3, false, $4, $4)
            ON CONFLICT (discord_id) DO UPDATE SET update_tm = $4
            RETURNING user_id
            ",
            user_discord_id.to_db_string(),
            user_language as i32,
            user_gender as i32,
            now
        )
        .fetch_one(&self.0)
        .await?
        .user_id)
    }

    /// target_discord_ids is used in a WHERE IN, so any duplicates are ignored
    #[instrument]
    pub async fn insert_emote_log(
        &self,
        user_discord_id: &UserId,
        guild_discord_id: Option<&GuildId>,
        target_discord_ids: impl Iterator<Item = &UserId> + std::fmt::Debug,
        emote_id: i32,
    ) -> Result<(), HandlerError> {
        debug!("inserting emote log");
        let now = time::OffsetDateTime::now_utc();
        let DbUser {
            language: user_language,
            gender: user_gender,
            ..
        } = DbUser::default();
        let user_id = self
            .upsert_user_not_set(user_discord_id, user_language, user_gender, now)
            .await?;

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
                gdi.to_db_string(),
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

        let emote_log_id = sqlx::query!(
            "
            INSERT INTO emote_logs (user_id, guild_id, emote_id, sent_at, insert_tm, update_tm)
            VALUES ($1, $2, $3, $4, $4, $4)
            RETURNING emote_log_id
            ",
            user_id,
            guild_id,
            emote_id,
            now
        )
        .fetch_one(&self.0)
        .await?
        .emote_log_id;

        // push_values below needs an iterator, so collect the upsert results first
        let user_ids: Vec<_> = stream::iter(target_discord_ids)
            .then(|id| async {
                self.upsert_user_not_set(id, user_language, user_gender, now)
                    .await
            })
            .try_collect()
            .await?;

        let mut query_builder =
            QueryBuilder::new("INSERT INTO emote_log_tags (emote_log_id, user_id) ");
        let mut is_empty = true;
        query_builder.push_values(user_ids.into_iter(), |mut builder, id| {
            is_empty = false;
            trace!("pushing mention {:?}", id.to_string());
            builder.push_bind(emote_log_id).push_bind(id);
        });
        if !is_empty {
            debug!("saving non-zero amount of mentions");
            query_builder.build().execute(&self.0).await?;
        }

        // sqlx::query!(
        //     "
        //     INSERT INTO emote_log_tags (emote_log_id, user_id)
        //     SELECT DISTINCT $1::bigint, users.user_id
        //     FROM users
        //     WHERE users.discord_id IN ($2)
        //     ",
        //     emote_log_id,
        //     target_discord_ids.map(|id| id.0).collect::<Vec<_>>()
        // )
        // .execute(&self.0)
        // .await?;

        Ok(())
    }

    pub async fn upsert_emotes(
        &self,
        emotes: impl Iterator<Item = (i32, String)>,
    ) -> Result<(), HandlerError> {
        debug!("upserting emotes");

        let now = time::OffsetDateTime::now_utc();
        for (id, command) in emotes {
            sqlx::query!(
                "
                INSERT INTO emotes (xiv_id, command, insert_tm, update_tm)
                VALUES ($1, $2, $3, $3)
                ON CONFLICT (xiv_id) DO UPDATE SET update_tm = $3
                ",
                id,
                command,
                now
            )
            .execute(&self.0)
            .await?;
        }

        Ok(())
    }

    pub async fn fetch_emote_log_count(
        &self,
        kind: impl Borrow<EmoteLogQuery>,
    ) -> Result<i64, HandlerError> {
        let res = match kind.borrow() {
            EmoteLogQuery::Guild(g) => {
                sqlx::query!(
                    "
                    SELECT COUNT(*) FROM emote_logs
                    JOIN guilds ON emote_logs.guild_id = guilds.guild_id
                    WHERE guilds.discord_id = $1
                    ",
                    g.to_string()
                )
                .fetch_one(&self.0)
                .await?
                .count
            }
            EmoteLogQuery::GuildUser((g, u)) => {
                sqlx::query!(
                    "
                    SELECT COUNT(*) FROM emote_logs
                    JOIN guilds on emote_logs.guild_id = guilds.guild_id
                    JOIN users on emote_logs.user_id = users.user_id
                    WHERE guilds.discord_id = $1 AND users.discord_id = $2
                    ",
                    g.to_string(),
                    u.to_string()
                )
                .fetch_one(&self.0)
                .await?
                .count
            }
            EmoteLogQuery::User(u) => {
                sqlx::query!(
                    "
                    SELECT COUNT(*) FROM emote_logs
                    JOIN users on emote_logs.user_id = users.user_id
                    WHERE users.discord_id = $1
                    ",
                    u.to_string()
                )
                .fetch_one(&self.0)
                .await?
                .count
            }
            EmoteLogQuery::ReceivedGuild(g) => {
                sqlx::query!(
                    "
                    SELECT COUNT(*) FROM emote_log_tags
                    JOIN emote_logs ON emote_log_tags.emote_log_id = emote_logs.emote_log_id
                    JOIN guilds on emote_logs.guild_id = guilds.guild_id
                    WHERE guilds.discord_id = $1
                    ",
                    g.to_string()
                )
                .fetch_one(&self.0)
                .await?
                .count
            }
            EmoteLogQuery::ReceivedGuildUser((g, u)) => {
                sqlx::query!(
                    "
                    SELECT COUNT(*) FROM emote_log_tags
                    JOIN emote_logs ON emote_log_tags.emote_log_id = emote_logs.emote_log_id
                    JOIN guilds on emote_logs.guild_id = guilds.guild_id
                    JOIN users on emote_logs.user_id = users.user_id
                    WHERE guilds.discord_id = $1 AND users.discord_id = $2
                    ",
                    g.to_string(),
                    u.to_string()
                )
                .fetch_one(&self.0)
                .await?
                .count
            }
            EmoteLogQuery::ReceivedUser(u) => {
                sqlx::query!(
                    "
                    SELECT COUNT(*) FROM emote_log_tags
                    JOIN emote_logs ON emote_log_tags.emote_log_id = emote_logs.emote_log_id
                    JOIN users on emote_logs.user_id = users.user_id
                    WHERE users.discord_id = $1
                    ",
                    u.to_string()
                )
                .fetch_one(&self.0)
                .await?
                .count
            }
        }
        .ok_or(HandlerError::CountNone)?;
        debug!("count for {:?}: {}", kind.borrow(), res);

        Ok(res)
    }
}
