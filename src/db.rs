use log::*;
use sqlx::PgPool;
use strum_macros::{EnumIter, FromRepr};
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

#[derive(sqlx::Type, Default, Debug, Clone, Copy, PartialEq, Eq, EnumIter, FromRepr)]
#[repr(i32)]
pub enum DbLanguage {
    #[default]
    En = 0,
    Ja = 1,
}

impl DbLanguage {
    pub fn to_string_en(self) -> &'static str {
        match self {
            DbLanguage::En => "English",
            DbLanguage::Ja => "Japanese",
        }
    }

    pub fn to_string_ja(self) -> &'static str {
        match self {
            DbLanguage::En => "英語",
            DbLanguage::Ja => "日本語",
        }
    }

    pub fn to_string(self, language: DbLanguage) -> &'static str {
        match language {
            DbLanguage::En => self.to_string_en(),
            DbLanguage::Ja => self.to_string_ja(),
        }
    }

    pub fn with_emote_data<'a>(&'a self, emote_data: &'a EmoteData) -> &LogMessagePair {
        match self {
            DbLanguage::En => &emote_data.en,
            DbLanguage::Ja => &emote_data.ja,
        }
    }
}

#[derive(sqlx::Type, Default, Debug, Clone, Copy, PartialEq, Eq, EnumIter, FromRepr)]
#[repr(i32)]
pub enum DbGender {
    #[default]
    M = 0,
    F = 1,
}

impl DbGender {
    pub fn to_string_en(self) -> &'static str {
        match self {
            DbGender::M => "Male",
            DbGender::F => "Female",
        }
    }

    pub fn to_string_ja(self) -> &'static str {
        match self {
            DbGender::M => "男性",
            DbGender::F => "女性",
        }
    }

    pub fn to_string(self, language: DbLanguage) -> &'static str {
        match language {
            DbLanguage::En => self.to_string_en(),
            DbLanguage::Ja => self.to_string_ja(),
        }
    }
}

impl From<DbGender> for Gender {
    fn from(g: DbGender) -> Self {
        match g {
            DbGender::M => Gender::Male,
            DbGender::F => Gender::Female,
        }
    }
}

#[derive(sqlx::FromRow, Debug, Clone)]
#[sqlx(type_name = "user")]
pub struct DbUser {
    pub discord_id: String,
    pub language: DbLanguage,
    pub gender: DbGender,
    pub insert_tm: time::OffsetDateTime,
    pub update_tm: time::OffsetDateTime,
}

impl Default for DbUser {
    fn default() -> Self {
        DbUser {
            discord_id: String::default(),
            language: DbLanguage::default(),
            gender: DbGender::default(),
            insert_tm: OffsetDateTime::now_utc(),
            update_tm: OffsetDateTime::now_utc(),
        }
    }
}

impl DbUser {
    pub fn discord_id(&self) -> &String {
        &self.discord_id
    }

    pub fn language(&self) -> &DbLanguage {
        &self.language
    }

    pub fn gender(&self) -> &DbGender {
        &self.gender
    }
}

#[derive(Debug, Clone)]
pub struct DbUserOpt(Option<DbUser>);

impl From<DbUserOpt> for Option<DbUser> {
    fn from(o: DbUserOpt) -> Self {
        o.into_inner()
    }
}

impl DbUserOpt {
    pub fn into_inner(self) -> Option<DbUser> {
        self.0
    }

    pub fn language(&self) -> DbLanguage {
        self.0
            .as_ref()
            .map(DbUser::language)
            .cloned()
            .unwrap_or_default()
    }

    pub fn gender(&self) -> DbGender {
        self.0
            .as_ref()
            .map(DbUser::gender)
            .cloned()
            .unwrap_or_default()
    }
}

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
