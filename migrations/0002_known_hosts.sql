CREATE TABLE known_hosts (
    hostname TEXT PRIMARY KEY,
    public_key TEXT NOT NULL,
    first_seen TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);
