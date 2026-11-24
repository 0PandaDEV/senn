CREATE TABLE account (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created INTEGER NOT NULL
);

CREATE TABLE agent (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    secret_hash TEXT NOT NULL,
    created INTEGER NOT NULL,
    stacks_dir TEXT NOT NULL,
    update_interval_secs INTEGER NOT NULL,
    auto_update INTEGER NOT NULL,
    last_seen INTEGER,
    info TEXT
);

CREATE TABLE session (
    token_hash TEXT PRIMARY KEY,
    expires INTEGER NOT NULL
);

CREATE INDEX session_expires ON session (expires);
