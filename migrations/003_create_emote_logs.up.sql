CREATE TABLE IF NOT EXISTS emotes (
	emote_id bigserial PRIMARY KEY,
	xiv_id int NOT NULL UNIQUE,
	command varchar(30) NOT NULL UNIQUE,
	insert_tm timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
	update_tm timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS emote_logs (
	emote_log_id bigserial PRIMARY KEY,
	user_id bigint NOT NULL,
	guild_id bigint,
	emote_id int NOT NULL,
	sent_at timestamptz NOT NULL,
	insert_tm timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
	update_tm timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
	FOREIGN KEY (user_id) REFERENCES users (user_id),
	FOREIGN KEY (guild_id) REFERENCES guilds (guild_id),
	FOREIGN KEY (emote_id) REFERENCES emotes (xiv_id)
);

ALTER TABLE users ADD is_set_flg boolean NOT NULL DEFAULT false;
ALTER TABLE guilds ADD is_set_flg boolean NOT NULL DEFAULT false;
