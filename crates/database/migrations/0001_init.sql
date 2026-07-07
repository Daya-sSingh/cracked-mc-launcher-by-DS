-- Migration 0001: core schema for the instance manager and global settings.
--
-- Later milestones add their own migrations on top of this one (accounts,
-- installed_mods, modrinth_cache, curseforge_cache, ...). sqlx's migrator
-- tracks which migrations have already run per-database, so these files are
-- append-only: never edit a migration that has shipped, only add new ones.

CREATE TABLE IF NOT EXISTS instances (
    id                      TEXT PRIMARY KEY NOT NULL,
    name                    TEXT NOT NULL,
    loader                  TEXT NOT NULL DEFAULT 'vanilla'
                                CHECK (loader IN ('vanilla', 'fabric')),
    loader_version          TEXT,
    minecraft_version       TEXT NOT NULL,
    icon                    TEXT,
    group_name              TEXT,
    favorite                INTEGER NOT NULL DEFAULT 0,

    java_path               TEXT,
    java_args               TEXT,
    memory_min_mb           INTEGER NOT NULL DEFAULT 1024,
    memory_max_mb           INTEGER NOT NULL DEFAULT 4096,
    window_width            INTEGER NOT NULL DEFAULT 854,
    window_height           INTEGER NOT NULL DEFAULT 480,
    fullscreen              INTEGER NOT NULL DEFAULT 0,
    game_args               TEXT,

    last_played_at          TEXT,
    total_playtime_seconds  INTEGER NOT NULL DEFAULT 0,

    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_instances_last_played
    ON instances (last_played_at DESC);

CREATE INDEX IF NOT EXISTS idx_instances_name
    ON instances (name COLLATE NOCASE);

-- Simple global key/value store for launcher-wide settings (theme, accent
-- color, default memory, bandwidth limits, etc). Values are JSON-encoded so
-- the Rust side can store arbitrarily shaped settings without a migration
-- for every new preference.
CREATE TABLE IF NOT EXISTS settings (
    key                     TEXT PRIMARY KEY NOT NULL,
    value                   TEXT NOT NULL,
    updated_at              TEXT NOT NULL
);
