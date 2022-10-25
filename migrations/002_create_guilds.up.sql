CREATE TABLE IF NOT EXISTS guilds (
	guild_id bigserial PRIMARY KEY,
	discord_id char(20) NOT NULL UNIQUE,
	language int NOT NULL,
	gender int NOT NULL,
	prefix varchar(5) NOT NULL,
	insert_tm timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
	update_tm timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP
);
