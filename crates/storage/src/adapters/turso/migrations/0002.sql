ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'administrator'));
ALTER TABLE users ADD COLUMN disabled_at INTEGER;
CREATE TABLE local_credentials (user_id TEXT PRIMARY KEY REFERENCES users(id), email TEXT NOT NULL UNIQUE, password_hash TEXT NOT NULL);
CREATE TABLE account_state (id INTEGER PRIMARY KEY CHECK (id = 1), initialized INTEGER NOT NULL DEFAULT 0, serial INTEGER NOT NULL DEFAULT 0);
INSERT INTO account_state (id) VALUES (1);
