CREATE TABLE devices (
  address TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 256),
  identity BLOB CHECK(identity IS NULL OR length(identity)=32)
);
CREATE TABLE history (
  id INTEGER PRIMARY KEY,
  address TEXT NOT NULL,
  occurred_at INTEGER NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('connected','disconnected','failed'))
);
CREATE TABLE settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
