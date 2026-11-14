use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::storage::error::StorageError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CipherItem {
    pub id: String,
    pub organization_id: Option<String>,
    pub folder_id: Option<String>,
    pub cipher_type: i32, // 1=Login, 2=SecureNote, 3=Card, 4=Identity
    pub name_enc: String,
    pub notes_enc: Option<String>,
    pub login_enc: Option<String>,
    pub card_enc: Option<String>,
    pub identity_enc: Option<String>,
    pub fields_enc: Option<String>,
    pub key_enc: Option<String>,
    pub favorite: bool,
    pub revision_date: i64,
    pub deleted_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecryptedCipherSummary {
    pub id: String,
    pub organization_id: Option<String>,
    pub folder_id: Option<String>,
    pub cipher_type: i32,
    pub name: String,
    pub username: Option<String>,
    pub uri: Option<String>,
    pub favorite: bool,
    pub revision_date: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderRecord {
    pub id: String,
    pub name_enc: String,
    pub revision_date: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationRecord {
    pub id: String,
    pub name: String,
    pub key_enc: String,
    pub use_totp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRecord {
    pub id: String,
    pub organization_id: String,
    pub name_enc: String,
    pub read_only: bool,
}

/// Open (or create) the vault database at the given path, running migrations.
pub fn open(path: &Path) -> Result<Connection, StorageError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<(), StorageError> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA foreign_keys=ON;

         CREATE TABLE IF NOT EXISTS ciphers (
             id TEXT PRIMARY KEY,
             organization_id TEXT,
             folder_id TEXT,
             type INTEGER NOT NULL,
             name_enc TEXT NOT NULL,
             notes_enc TEXT,
             login_enc TEXT,
             card_enc TEXT,
             identity_enc TEXT,
             fields_enc TEXT,
             key_enc TEXT,
             favorite INTEGER NOT NULL DEFAULT 0,
             revision_date INTEGER NOT NULL,
             deleted_date INTEGER
         );

         CREATE INDEX IF NOT EXISTS idx_ciphers_folder ON ciphers(folder_id);
         CREATE INDEX IF NOT EXISTS idx_ciphers_org ON ciphers(organization_id);

         CREATE TABLE IF NOT EXISTS folders (
             id TEXT PRIMARY KEY,
             name_enc TEXT NOT NULL,
             revision_date INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS organizations (
             id TEXT PRIMARY KEY,
             name TEXT NOT NULL,
             key_enc TEXT NOT NULL,
             use_totp INTEGER NOT NULL DEFAULT 0
         );

         CREATE TABLE IF NOT EXISTS collections (
             id TEXT PRIMARY KEY,
             organization_id TEXT NOT NULL,
             name_enc TEXT NOT NULL,
             read_only INTEGER NOT NULL DEFAULT 0,
             FOREIGN KEY(organization_id) REFERENCES organizations(id) ON DELETE CASCADE
         );

         CREATE TABLE IF NOT EXISTS cipher_collections (
             cipher_id TEXT NOT NULL,
             collection_id TEXT NOT NULL,
             PRIMARY KEY(cipher_id, collection_id),
             FOREIGN KEY(cipher_id) REFERENCES ciphers(id) ON DELETE CASCADE,
             FOREIGN KEY(collection_id) REFERENCES collections(id) ON DELETE CASCADE
         );

         CREATE TABLE IF NOT EXISTS attachments (
             id TEXT PRIMARY KEY,
             cipher_id TEXT NOT NULL,
             file_name_enc TEXT NOT NULL,
             key_enc TEXT NOT NULL,
             size INTEGER NOT NULL,
             local_path TEXT,
             FOREIGN KEY(cipher_id) REFERENCES ciphers(id) ON DELETE CASCADE
         );

         -- Decrypted search cache for fast sub-millisecond filtering
         CREATE TABLE IF NOT EXISTS search_cache (
             cipher_id TEXT PRIMARY KEY,
             name TEXT NOT NULL,
             username TEXT,
             uri TEXT,
             FOREIGN KEY(cipher_id) REFERENCES ciphers(id) ON DELETE CASCADE
         );

         CREATE VIRTUAL TABLE IF NOT EXISTS ciphers_fts USING fts5(
             name,
             username,
             uri,
             content='search_cache',
             content_rowid='rowid'
         );

         CREATE TRIGGER IF NOT EXISTS search_cache_ai AFTER INSERT ON search_cache BEGIN
             INSERT INTO ciphers_fts(rowid, name, username, uri)
             VALUES (new.rowid, new.name, new.username, new.uri);
         END;

         CREATE TRIGGER IF NOT EXISTS search_cache_ad AFTER DELETE ON search_cache BEGIN
             INSERT INTO ciphers_fts(ciphers_fts, rowid, name, username, uri)
             VALUES ('delete', old.rowid, old.name, old.username, old.uri);
         END;

         CREATE TRIGGER IF NOT EXISTS search_cache_au AFTER UPDATE ON search_cache BEGIN
             INSERT INTO ciphers_fts(ciphers_fts, rowid, name, username, uri)
             VALUES ('delete', old.rowid, old.name, old.username, old.uri);
             INSERT INTO ciphers_fts(rowid, name, username, uri)
             VALUES (new.rowid, new.name, new.username, new.uri);
         END;

         CREATE TABLE IF NOT EXISTS metadata (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );",
    )
    .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

    Ok(())
}

/// Upsert a cipher and update its decrypted search cache.
pub fn upsert_cipher(
    conn: &Connection,
    item: &CipherItem,
    decrypted_name: &str,
    decrypted_username: Option<&str>,
    decrypted_uri: Option<&str>,
) -> Result<(), StorageError> {
    conn.execute(
        "INSERT INTO ciphers (id, organization_id, folder_id, type, name_enc, notes_enc, login_enc, card_enc, identity_enc, fields_enc, key_enc, favorite, revision_date, deleted_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(id) DO UPDATE SET
             organization_id=excluded.organization_id,
             folder_id=excluded.folder_id,
             type=excluded.type,
             name_enc=excluded.name_enc,
             notes_enc=excluded.notes_enc,
             login_enc=excluded.login_enc,
             card_enc=excluded.card_enc,
             identity_enc=excluded.identity_enc,
             fields_enc=excluded.fields_enc,
             key_enc=excluded.key_enc,
             favorite=excluded.favorite,
             revision_date=excluded.revision_date,
             deleted_date=excluded.deleted_date",
        params![
            item.id,
            item.organization_id,
            item.folder_id,
            item.cipher_type,
            item.name_enc,
            item.notes_enc,
            item.login_enc,
            item.card_enc,
            item.identity_enc,
            item.fields_enc,
            item.key_enc,
            if item.favorite { 1 } else { 0 },
            item.revision_date,
            item.deleted_date
        ],
    )
    .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

    conn.execute(
        "INSERT INTO search_cache (cipher_id, name, username, uri)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(cipher_id) DO UPDATE SET
             name=excluded.name,
             username=excluded.username,
             uri=excluded.uri",
        params![item.id, decrypted_name, decrypted_username, decrypted_uri],
    )
    .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

    Ok(())
}

pub fn delete_cipher(conn: &Connection, id: &str) -> Result<(), StorageError> {
    conn.execute("DELETE FROM ciphers WHERE id=?1", params![id])
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    conn.execute("DELETE FROM search_cache WHERE cipher_id=?1", params![id])
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    Ok(())
}

pub fn get_cipher_by_id(conn: &Connection, id: &str) -> Result<CipherItem, StorageError> {
    conn.query_row(
        "SELECT id, organization_id, folder_id, type, name_enc, notes_enc, login_enc, card_enc, identity_enc, fields_enc, key_enc, favorite, revision_date, deleted_date
         FROM ciphers WHERE id = ?1",
        params![id],
        |row| {
            let fav: i32 = row.get(11)?;
            Ok(CipherItem {
                id: row.get(0)?,
                organization_id: row.get(1)?,
                folder_id: row.get(2)?,
                cipher_type: row.get(3)?,
                name_enc: row.get(4)?,
                notes_enc: row.get(5)?,
                login_enc: row.get(6)?,
                card_enc: row.get(7)?,
                identity_enc: row.get(8)?,
                fields_enc: row.get(9)?,
                key_enc: row.get(10)?,
                favorite: fav != 0,
                revision_date: row.get(12)?,
                deleted_date: row.get(13)?,
            })
        },
    )
    .map_err(|_| StorageError::NotFound(id.to_string()))
}

/// Search ciphers via FTS5 index.
pub fn search_ciphers(
    conn: &Connection,
    query: &str,
    folder_id: Option<&str>,
    organization_id: Option<&str>,
) -> Result<Vec<DecryptedCipherSummary>, StorageError> {
    let trimmed = query.trim();

    let sql = if trimmed.is_empty() {
        "SELECT c.id, c.organization_id, c.folder_id, c.type, s.name, s.username, s.uri, c.favorite, c.revision_date
         FROM ciphers c
         JOIN search_cache s ON c.id = s.cipher_id
         WHERE c.deleted_date IS NULL
           AND (?1 IS NULL OR c.folder_id = ?1)
           AND (?2 IS NULL OR c.organization_id = ?2)
         ORDER BY s.name COLLATE NOCASE ASC"
    } else {
        "SELECT c.id, c.organization_id, c.folder_id, c.type, s.name, s.username, s.uri, c.favorite, c.revision_date
         FROM ciphers c
         JOIN search_cache s ON c.id = s.cipher_id
         JOIN ciphers_fts f ON s.rowid = f.rowid
         WHERE ciphers_fts MATCH ?3
           AND c.deleted_date IS NULL
           AND (?1 IS NULL OR c.folder_id = ?1)
           AND (?2 IS NULL OR c.organization_id = ?2)
         ORDER BY rank"
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

    let escaped = trimmed.replace('"', "\"\"");
    let fts_query = format!("\"{escaped}\"*");

    let rows = stmt
        .query_map(params![folder_id, organization_id, fts_query], |row| {
            let fav: i32 = row.get(7)?;
            Ok(DecryptedCipherSummary {
                id: row.get(0)?,
                organization_id: row.get(1)?,
                folder_id: row.get(2)?,
                cipher_type: row.get(3)?,
                name: row.get(4)?,
                username: row.get(5)?,
                uri: row.get(6)?,
                favorite: fav != 0,
                revision_date: row.get(8)?,
            })
        })
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

    Ok(rows)
}

pub fn upsert_folder(conn: &Connection, folder: &FolderRecord) -> Result<(), StorageError> {
    conn.execute(
        "INSERT INTO folders (id, name_enc, revision_date)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET
             name_enc=excluded.name_enc,
             revision_date=excluded.revision_date",
        params![folder.id, folder.name_enc, folder.revision_date],
    )
    .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    Ok(())
}

pub fn list_folders(conn: &Connection) -> Result<Vec<FolderRecord>, StorageError> {
    let mut stmt = conn
        .prepare("SELECT id, name_enc, revision_date FROM folders ORDER BY id")
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FolderRecord {
                id: row.get(0)?,
                name_enc: row.get(1)?,
                revision_date: row.get(2)?,
            })
        })
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    Ok(rows)
}

pub fn delete_folder(conn: &Connection, id: &str) -> Result<(), StorageError> {
    conn.execute("UPDATE ciphers SET folder_id = NULL WHERE folder_id = ?1", params![id])
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    conn.execute("DELETE FROM folders WHERE id = ?1", params![id])
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    Ok(())
}

pub fn upsert_organization(conn: &Connection, org: &OrganizationRecord) -> Result<(), StorageError> {
    conn.execute(
        "INSERT INTO organizations (id, name, key_enc, use_totp)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
             name=excluded.name,
             key_enc=excluded.key_enc,
             use_totp=excluded.use_totp",
        params![org.id, org.name, org.key_enc, if org.use_totp { 1 } else { 0 }],
    )
    .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    Ok(())
}

pub fn list_organizations(conn: &Connection) -> Result<Vec<OrganizationRecord>, StorageError> {
    let mut stmt = conn
        .prepare("SELECT id, name, key_enc, use_totp FROM organizations ORDER BY name")
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    let rows = stmt
        .query_map([], |row| {
            let use_totp: i32 = row.get(3)?;
            Ok(OrganizationRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                key_enc: row.get(2)?,
                use_totp: use_totp != 0,
            })
        })
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    Ok(rows)
}

pub fn upsert_collection(conn: &Connection, col: &CollectionRecord) -> Result<(), StorageError> {
    conn.execute(
        "INSERT INTO collections (id, organization_id, name_enc, read_only)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
             organization_id=excluded.organization_id,
             name_enc=excluded.name_enc,
             read_only=excluded.read_only",
        params![col.id, col.organization_id, col.name_enc, if col.read_only { 1 } else { 0 }],
    )
    .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    Ok(())
}

pub fn list_collections(conn: &Connection, org_id: Option<&str>) -> Result<Vec<CollectionRecord>, StorageError> {
    let sql = "SELECT id, organization_id, name_enc, read_only FROM collections WHERE (?1 IS NULL OR organization_id = ?1)";
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    let rows = stmt
        .query_map(params![org_id], |row| {
            let ro: i32 = row.get(3)?;
            Ok(CollectionRecord {
                id: row.get(0)?,
                organization_id: row.get(1)?,
                name_enc: row.get(2)?,
                read_only: ro != 0,
            })
        })
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
        .collect::<SqlResult<Vec<_>>>()
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    Ok(rows)
}

pub fn get_metadata(conn: &Connection, key: &str) -> Result<Option<String>, StorageError> {
    let mut stmt = conn
        .prepare("SELECT value FROM metadata WHERE key=?1")
        .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    let result: SqlResult<String> = stmt.query_row(params![key], |r| r.get(0));
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))),
    }
}

pub fn set_metadata(conn: &Connection, key: &str, value: &str) -> Result<(), StorageError> {
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )
    .map_err(|e| StorageError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn
    }

    #[test]
    fn test_cipher_upsert_and_fts_search() {
        let conn = in_memory_db();
        let cipher = CipherItem {
            id: "cipher-1".into(),
            organization_id: None,
            folder_id: None,
            cipher_type: 1, // Login
            name_enc: "2.name_enc...".into(),
            notes_enc: None,
            login_enc: Some("2.login_enc...".into()),
            card_enc: None,
            identity_enc: None,
            fields_enc: None,
            key_enc: None,
            favorite: false,
            revision_date: 1700000000,
            deleted_date: None,
        };

        upsert_cipher(
            &conn,
            &cipher,
            "GitHub Personal",
            Some("tegar_user"),
            Some("https://github.com"),
        )
        .unwrap();

        // Exact match
        let res = search_ciphers(&conn, "GitHub", None, None).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "cipher-1");
        assert_eq!(res[0].name, "GitHub Personal");

        // Username search
        let res_user = search_ciphers(&conn, "tegar", None, None).unwrap();
        assert_eq!(res_user.len(), 1);

        // Delete cipher
        delete_cipher(&conn, "cipher-1").unwrap();
        let res_empty = search_ciphers(&conn, "GitHub", None, None).unwrap();
        assert_eq!(res_empty.len(), 0);
    }
}
