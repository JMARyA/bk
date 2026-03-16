CREATE TABLE forget_events (
    id          BIGSERIAL PRIMARY KEY,
    hostname    TEXT NOT NULL,
    target      TEXT NOT NULL,
    removed     BIGINT NOT NULL DEFAULT 0,
    kept        BIGINT NOT NULL DEFAULT 0,
    dry_run     BOOLEAN NOT NULL DEFAULT false,
    timestamp   TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);
