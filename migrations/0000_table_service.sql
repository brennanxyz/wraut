CREATE TABLE service (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    compose_name TEXT NOT NULL,
    repo_url TEXT NOT NULL,
    access_url TEXT NOT NULL,
    active bool NOT NULL DEFAULT false,
    use_key bool NOT NULL DEFAULT false
);
