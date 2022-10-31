pub mod models;
pub mod util;

use std::borrow::Borrow;
use std::sync::Arc;

use futures::{stream, StreamExt, TryStreamExt};
use serenity::model::prelude::{GuildId, UserId};
use sqlx::{PgPool, QueryBuilder, Row};
use tracing::*;
use xiv_emote_parser::repository::EmoteData;

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
    ) -> Result<i64, HandlerError> {
        debug!("upserting user");
        if let Some(rec) = sqlx::query!(
            "
            SELECT user_id FROM users WHERE discord_id = $1
            ",
            discord_id.to_db_string()
        )
        .fetch_optional(&self.0)
        .await?
        {
            Ok(rec.user_id)
        } else {
            self.upsert_user_with_is_set(
                discord_id,
                language,
                gender,
                true,
                time::OffsetDateTime::now_utc(),
            )
            .await
        }
    }

    async fn upsert_user_not_set(
        &self,
        discord_id: &UserId,
        language: DbLanguage,
        gender: DbGender,
        now: time::OffsetDateTime,
    ) -> Result<i64, HandlerError> {
        if let Some(rec) = sqlx::query!(
            "
            SELECT user_id FROM users WHERE discord_id = $1
            ",
            discord_id.to_db_string()
        )
        .fetch_optional(&self.0)
        .await?
        {
            Ok(rec.user_id)
        } else {
            self.upsert_user_with_is_set(discord_id, language, gender, false, now)
                .await
        }
    }

    pub async fn upsert_user_with_is_set(
        &self,
        discord_id: &UserId,
        language: DbLanguage,
        gender: DbGender,
        is_set_flg: bool,
        now: time::OffsetDateTime,
    ) -> Result<i64, HandlerError> {
        Ok(sqlx::query!(
            "
            INSERT INTO users (discord_id, language, gender, is_set_flg, insert_tm, update_tm)
            VALUES ($1, $2, $3, $4, $5, $5)
            RETURNING user_id
            ",
            discord_id.to_db_string(),
            language as i32,
            gender as i32,
            is_set_flg,
            now
        )
        .fetch_one(&self.0)
        .await?
        .user_id)
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
    ) -> Result<i64, HandlerError> {
        debug!("upserting guild");
        if let Some(rec) = sqlx::query!(
            "
            SELECT guild_id FROM guilds WHERE discord_id = $1
            ",
            discord_id.to_db_string()
        )
        .fetch_optional(&self.0)
        .await?
        {
            Ok(rec.guild_id)
        } else {
            self.upsert_guild_with_is_set(
                discord_id,
                language,
                gender,
                prefix,
                true,
                time::OffsetDateTime::now_utc(),
            )
            .await
        }
    }

    async fn upsert_guild_not_set(
        &self,
        discord_id: &GuildId,
        language: DbLanguage,
        gender: DbGender,
        prefix: String,
        now: time::OffsetDateTime,
    ) -> Result<i64, HandlerError> {
        if let Some(rec) = sqlx::query!(
            "
            SELECT guild_id FROM guilds WHERE discord_id = $1
            ",
            discord_id.to_db_string()
        )
        .fetch_optional(&self.0)
        .await?
        {
            Ok(rec.guild_id)
        } else {
            self.upsert_guild_with_is_set(discord_id, language, gender, prefix, false, now)
                .await
        }
    }

    pub async fn upsert_guild_with_is_set(
        &self,
        discord_id: &GuildId,
        language: DbLanguage,
        gender: DbGender,
        prefix: String,
        is_set_flg: bool,
        now: time::OffsetDateTime,
    ) -> Result<i64, HandlerError> {
        Ok(sqlx::query!(
            "
            INSERT INTO guilds (discord_id, language, gender, prefix, is_set_flg, insert_tm, update_tm)
            VALUES ($1, $2, $3, $4, $5, $6, $6)
            RETURNING guild_id
            ",
            discord_id.to_db_string(),
            language as i32,
            gender as i32,
            prefix,
            is_set_flg,
            now
        )
        .fetch_one(&self.0)
        .await?
        .guild_id)
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
            Some(
                self.upsert_guild_not_set(gdi, guild_language, guild_gender, guild_prefix, now)
                    .await?,
            )
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

        // push_values below needs an iterator, not a stream, so collect the upsert results first
        let user_ids: Vec<_> = stream::iter(target_discord_ids)
            .then(|id| async {
                self.upsert_user_not_set(id, user_language, user_gender, now)
                    .await
            })
            .try_collect()
            .await?;

        if !user_ids.is_empty() {
            let mut query_builder =
                QueryBuilder::new("INSERT INTO emote_log_tags (emote_log_id, user_id) ");
            query_builder.push_values(user_ids.into_iter(), |mut builder, id| {
                trace!("pushing mention {:?}", id.to_string());
                builder.push_bind(emote_log_id).push_bind(id);
            });
            debug!("saving mentions");
            query_builder.build().execute(&self.0).await?;
        }

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

    async fn try_add_emote_condition<'a>(
        &self,
        query_builder: &mut QueryBuilder<'a, sqlx::Postgres>,
        em_opt: &'a Option<Arc<EmoteData>>,
    ) -> Result<(), HandlerError> {
        if let Some(em) = em_opt {
            let emote_id = sqlx::query!(
                "
                SELECT emote_id FROM emotes WHERE xiv_id = $1
                ",
                em.id as i32
            )
            .fetch_one(&self.0)
            .await?
            .emote_id;
            query_builder
                .push(" AND emote_logs.emote_id = ")
                .push_bind(emote_id);
        }
        Ok(())
    }

    pub async fn fetch_emote_log_count(
        &self,
        kind: impl Borrow<EmoteLogQuery>,
    ) -> Result<i64, HandlerError> {
        let mut query_builder = QueryBuilder::new("SELECT COUNT(*) FROM emote_logs ");
        match kind.borrow() {
            EmoteLogQuery::Guild((g, em_opt)) => {
                query_builder
                    .push(
                        "
                        JOIN guilds ON emote_logs.guild_id = guilds.guild_id
                        WHERE guilds.discord_id = ",
                    )
                    .push_bind(g.to_db_string());
                self.try_add_emote_condition(&mut query_builder, em_opt)
                    .await?;
            }
            EmoteLogQuery::GuildUser((g, u, em_opt)) => {
                query_builder
                    .push(
                        "
                        JOIN guilds on emote_logs.guild_id = guilds.guild_id
                        JOIN users on emote_logs.user_id = users.user_id
                        WHERE guilds.discord_id = ",
                    )
                    .push_bind(g.to_db_string())
                    .push(" AND users.discord_id = ")
                    .push_bind(u.to_db_string());
                self.try_add_emote_condition(&mut query_builder, em_opt)
                    .await?;
            }
            EmoteLogQuery::User((u, em_opt)) => {
                query_builder
                    .push(
                        "
                        JOIN users on emote_logs.user_id = users.user_id
                        WHERE users.discord_id = ",
                    )
                    .push_bind(u.to_db_string());
                self.try_add_emote_condition(&mut query_builder, em_opt)
                    .await?;
            }
            EmoteLogQuery::ReceivedGuild((g, em_opt)) => {
                query_builder
                    .push(
                        "
                        JOIN emote_logs ON emote_log_tags.emote_log_id = emote_logs.emote_log_id
                        JOIN guilds on emote_logs.guild_id = guilds.guild_id
                        WHERE guilds.discord_id = ",
                    )
                    .push_bind(g.to_db_string());
                self.try_add_emote_condition(&mut query_builder, em_opt)
                    .await?;
            }
            EmoteLogQuery::ReceivedGuildUser((g, u, em_opt)) => {
                query_builder
                    .push(
                        "
                        JOIN emote_logs ON emote_log_tags.emote_log_id = emote_logs.emote_log_id
                        JOIN guilds on emote_logs.guild_id = guilds.guild_id
                        JOIN users on emote_logs.user_id = users.user_id
                        WHERE guilds.discord_id = ",
                    )
                    .push_bind(g.to_db_string())
                    .push(" AND users.user_id = ")
                    .push_bind(u.to_db_string());
                self.try_add_emote_condition(&mut query_builder, em_opt)
                    .await?;
            }
            EmoteLogQuery::ReceivedUser((u, em_opt)) => {
                query_builder
                    .push(
                        "
                        JOIN emote_logs ON emote_log_tags.emote_log_id = emote_logs.emote_log_id
                        JOIN users on emote_logs.user_id = users.user_id
                        WHERE users.discord_id = ",
                    )
                    .push_bind(u.to_db_string());
                self.try_add_emote_condition(&mut query_builder, em_opt)
                    .await?;
            }
        }

        let res: i64 = query_builder.build().fetch_one(&self.0).await?.get(0);
        debug!("count for {:?}: {}", kind.borrow(), res);

        Ok(res)
    }
}
