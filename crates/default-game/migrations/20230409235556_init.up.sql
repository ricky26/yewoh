CREATE TABLE world_snapshots (
    shard_id TEXT PRIMARY KEY,
    snapshot BYTEA,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE accounts (
    username TEXT NOT NULL PRIMARY KEY,
    password_hash TEXT NOT NULL,
    character_slots INTEGER NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE characters (
    id UUID NOT NULL PRIMARY KEY,
    username TEXT NOT NULL REFERENCES accounts(username),
    slot INTEGER NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX accounts_characters ON characters (username, slot) INCLUDE (id);
