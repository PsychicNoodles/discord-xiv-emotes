CREATE TABLE IF NOT EXISTS emote_log_tags (
    emote_log_id bigint NOT NULL,
    user_id bigint NOT NULL,
	insert_tm timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
	update_tm timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (emote_log_id) REFERENCES emote_logs (emote_log_id),
    FOREIGN KEY (user_id) REFERENCES users (user_id)
);

CREATE UNIQUE INDEX ON emote_log_tags (emote_log_id, user_id);
