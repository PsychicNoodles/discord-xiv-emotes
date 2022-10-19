use log::*;
use sqlx::PgPool;
use thiserror::Error;
use time::OffsetDateTime;
use xiv_emote_parser::{
    log_message::condition::Gender,
    repository::{EmoteData, LogMessagePair},
};

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database error ({0})")]
    Db(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(sqlx::Type, Default, Debug, Clone, Copy)]
#[repr(i32)]
pub enum DbUserLanguage {
    #[default]
    En = 0,
    Ja = 1,
}

impl DbUserLanguage {
    pub fn with_emote_data<'a>(&'a self, emote_data: &'a EmoteData) -> &LogMessagePair {
        match self {
            DbUserLanguage::En => &emote_data.en,
            DbUserLanguage::Ja => &emote_data.ja,
        }
    }
}

#[derive(sqlx::Type, Default, Debug, Clone, Copy)]
#[repr(i32)]
pub enum DbUserGender {
    #[default]
    M = 0,
    F = 1,
}

impl From<DbUserGender> for Gender {
    fn from(g: DbUserGender) -> Self {
        match g {
            DbUserGender::M => Gender::Male,
            DbUserGender::F => Gender::Female,
        }
    }
}

#[derive(sqlx::FromRow, Debug, Clone)]
#[sqlx(type_name = "user")]
pub struct DbUser {
    pub discord_id: String,
    pub language: DbUserLanguage,
    pub gender: DbUserGender,
    pub insert_tm: time::OffsetDateTime,
    pub update_tm: time::OffsetDateTime,
}

impl Default for DbUser {
    fn default() -> Self {
        DbUser {
            discord_id: String::default(),
            language: DbUserLanguage::default(),
            gender: DbUserGender::default(),
            insert_tm: OffsetDateTime::now_utc(),
            update_tm: OffsetDateTime::now_utc(),
        }
    }
}

impl DbUser {
    pub fn discord_id(&self) -> &String {
        &self.discord_id
    }

    pub fn language(&self) -> &DbUserLanguage {
        &self.language
    }

    pub fn gender(&self) -> &DbUserGender {
        &self.gender
    }
}

#[derive(Debug)]
pub struct Db(pub PgPool);

impl Db {
    pub async fn insert_user(
        &self,
        discord_id: String,
        language: DbUserLanguage,
        gender: DbUserGender,
    ) -> Result<()> {
        debug!("inserting user {} {:?} {:?}", discord_id, language, gender);
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

    pub async fn find_user(&self, discord_id: impl AsRef<str>) -> Result<Option<DbUser>> {
        let discord_id = discord_id.as_ref();
        debug!("checking for user {:?}", discord_id);
        let res = sqlx::query_as!(
            DbUser,
            r#"
            SELECT
                discord_id,
                language as "language: DbUserLanguage",
                gender as "gender: DbUserGender",
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
}
