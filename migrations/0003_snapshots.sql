CREATE TABLE snapshots (
    id          TEXT PRIMARY KEY,
    short_id    TEXT,
    hostname    TEXT NOT NULL,
    paths       TEXT NOT NULL,
    tags        TEXT,
    time        TIMESTAMP WITH TIME ZONE NOT NULL,
    username    TEXT,
    received_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);
