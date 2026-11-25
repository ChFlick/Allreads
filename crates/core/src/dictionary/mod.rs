//! Offline dictionaries: the file format shared by `dictgen` (which writes
//! it) and the app (which looks words up in it).

/// The tables of a dictionary file. Indexes are created by
/// [`INDEXES`] after the bulk insert, which is much faster.
pub const SCHEMA: &str = include_str!("schema.sql");

/// Indexes for lookups by term key.
/// (`form` needs none: its primary key starts with `key`.)
pub const INDEXES: &str = "CREATE INDEX entry_key ON entry (key);";

/// Bumped whenever [`SCHEMA`] changes; stored in `meta` as `format`.
pub const FORMAT_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_and_indexes_create_cleanly() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch(INDEXES).unwrap();
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 4);
    }
}
