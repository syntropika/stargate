CREATE TABLE users (id TEXT PRIMARY KEY, email TEXT, created_at INTEGER NOT NULL, data TEXT NOT NULL);
CREATE TABLE identities (id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id), issuer TEXT NOT NULL, subject TEXT NOT NULL, email TEXT, metadata TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(issuer, subject));
CREATE TABLE sessions (id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id), token_hash TEXT NOT NULL UNIQUE, expires_at INTEGER NOT NULL, revoked_at INTEGER, last_used_at INTEGER, data TEXT NOT NULL);
CREATE INDEX sessions_owner ON sessions(user_id);
CREATE TABLE api_keys (id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id), secret_hash TEXT NOT NULL UNIQUE, expires_at INTEGER, revoked_at INTEGER, last_used_at INTEGER, data TEXT NOT NULL);
CREATE INDEX api_keys_owner ON api_keys(user_id);
CREATE TABLE oidc_transactions (state_hash TEXT PRIMARY KEY, browser_hash TEXT NOT NULL, expires_at INTEGER NOT NULL, data TEXT NOT NULL);
CREATE TABLE audit_events (id TEXT PRIMARY KEY, event TEXT NOT NULL, actor TEXT, created_at INTEGER NOT NULL, data TEXT NOT NULL);
