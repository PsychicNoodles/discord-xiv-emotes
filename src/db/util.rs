use serenity::model::prelude::{GuildId, RoleId, UserId};

pub trait DiscordIdExt {
    fn to_db_string(&self) -> String;
}

impl DiscordIdExt for &UserId {
    fn to_db_string(&self) -> String {
        format!("{:0>20}", self.0)
    }
}

impl DiscordIdExt for UserId {
    fn to_db_string(&self) -> String {
        format!("{:0>20}", self.0)
    }
}

impl DiscordIdExt for &GuildId {
    fn to_db_string(&self) -> String {
        format!("{:0>20}", self.0)
    }
}

impl DiscordIdExt for GuildId {
    fn to_db_string(&self) -> String {
        format!("{:0>20}", self.0)
    }
}

impl DiscordIdExt for &RoleId {
    fn to_db_string(&self) -> String {
        format!("{:0>20}", self.0)
    }
}

impl DiscordIdExt for RoleId {
    fn to_db_string(&self) -> String {
        format!("{:0>20}", self.0)
    }
}
