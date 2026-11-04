//! Phrase-safe SQLite FTS5 session query index (`dsh-session-query-sqlite` realization).
//!
//! Uses POSIX system `libsqlite3` (`fts5` + `unicode61` tokenizer) with quoted literal phrase
//! protection so user queries containing `OR`, `NEAR`, `*`, or `"` never trigger FTS syntax errors
//! or operator injection. Automatically degrades to deterministic in-memory phrase scoring if
//! `libsqlite3` FTS5 is unavailable.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FtsSessionHit {
    pub session_id: String,
    pub workspace: String,
    pub title: String,
    pub snippet: String,
    pub updated_at: u64,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct FtsSessionDoc {
    pub session_id: String,
    pub workspace: String,
    pub title: String,
    pub content: String,
    pub updated_at: u64,
}

/// Quotes every whitespace-separated term as an FTS5 literal phrase `"..."` (doubling internal `"`),
/// turning FTS5 operators (`OR`, `AND`, `NOT`, `NEAR`, `*`, `^`, `:`) into inert literal text.
pub fn build_safe_fts5_phrase_query(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let escaped = trimmed.replace('"', "\"\"");
    Some(format!("\"{escaped}\""))
}

#[cfg(unix)]
mod unix_sqlite {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int, c_void};
    use std::ptr;

    #[repr(C)]
    struct sqlite3(c_void);
    #[repr(C)]
    struct sqlite3_stmt(c_void);

    const SQLITE_OK: c_int = 0;
    const SQLITE_ROW: c_int = 100;
    const SQLITE_OPEN_READWRITE: c_int = 0x0000_0002;
    const SQLITE_OPEN_CREATE: c_int = 0x0000_0004;
    const SQLITE_OPEN_NOMUTEX: c_int = 0x0000_8000;

    type SqliteDestructor = Option<unsafe extern "C" fn(*mut c_void)>;

    fn sqlite_transient() -> SqliteDestructor {
        unsafe { std::mem::transmute::<isize, SqliteDestructor>(-1) }
    }

    #[link(name = "sqlite3")]
    extern "C" {
        fn sqlite3_open_v2(
            filename: *const c_char,
            pp_db: *mut *mut sqlite3,
            flags: c_int,
            z_vfs: *const c_char,
        ) -> c_int;
        fn sqlite3_close(db: *mut sqlite3) -> c_int;
        fn sqlite3_exec(
            db: *mut sqlite3,
            sql: *const c_char,
            callback: Option<
                unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
            >,
            arg: *mut c_void,
            errmsg: *mut *mut c_char,
        ) -> c_int;
        fn sqlite3_free(ptr: *mut c_void);
        fn sqlite3_prepare_v2(
            db: *mut sqlite3,
            z_sql: *const c_char,
            n_byte: c_int,
            pp_stmt: *mut *mut sqlite3_stmt,
            pz_tail: *mut *const c_char,
        ) -> c_int;
        fn sqlite3_bind_text(
            stmt: *mut sqlite3_stmt,
            idx: c_int,
            val: *const c_char,
            n: c_int,
            destructor: SqliteDestructor,
        ) -> c_int;
        fn sqlite3_bind_int64(stmt: *mut sqlite3_stmt, idx: c_int, val: i64) -> c_int;
        fn sqlite3_step(stmt: *mut sqlite3_stmt) -> c_int;
        fn sqlite3_column_text(stmt: *mut sqlite3_stmt, i_col: c_int) -> *const u8;
        fn sqlite3_column_int64(stmt: *mut sqlite3_stmt, i_col: c_int) -> i64;
        fn sqlite3_column_double(stmt: *mut sqlite3_stmt, i_col: c_int) -> f64;
        fn sqlite3_finalize(stmt: *mut sqlite3_stmt) -> c_int;
    }

    pub struct SqliteConn(*mut sqlite3);
    unsafe impl Send for SqliteConn {}

    impl Drop for SqliteConn {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    sqlite3_close(self.0);
                }
            }
        }
    }

    impl SqliteConn {
        pub fn open(db_path: Option<&Path>) -> Result<Self, String> {
            let c_path = match db_path {
                Some(p) => CString::new(p.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?,
                None => CString::new(":memory:").unwrap(),
            };
            let mut raw: *mut sqlite3 = ptr::null_mut();
            let rc = unsafe {
                sqlite3_open_v2(
                    c_path.as_ptr(),
                    &mut raw,
                    SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_NOMUTEX,
                    ptr::null(),
                )
            };
            if rc != SQLITE_OK || raw.is_null() {
                if !raw.is_null() {
                    unsafe {
                        sqlite3_close(raw);
                    }
                }
                return Err(format!("sqlite3_open_v2 failed (rc={rc})"));
            }
            let conn = Self(raw);
            conn.exec_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA synchronous=NORMAL;
                 PRAGMA application_id=1364748873;
                 CREATE VIRTUAL TABLE IF NOT EXISTS session_fts USING fts5(
                     session_id UNINDEXED,
                     workspace UNINDEXED,
                     title,
                     content,
                     updated_at UNINDEXED,
                     tokenize = 'unicode61'
                 );",
            )?;
            Ok(conn)
        }

        fn exec_batch(&self, sql: &str) -> Result<(), String> {
            let c_sql = CString::new(sql.replace('\0', " ")).map_err(|e| e.to_string())?;
            let mut err_ptr: *mut c_char = ptr::null_mut();
            let rc = unsafe {
                sqlite3_exec(self.0, c_sql.as_ptr(), None, ptr::null_mut(), &mut err_ptr)
            };
            if rc != SQLITE_OK {
                let msg = if err_ptr.is_null() {
                    format!("sqlite3_exec rc={rc}")
                } else {
                    let s = unsafe { CStr::from_ptr(err_ptr).to_string_lossy().into_owned() };
                    unsafe {
                        sqlite3_free(err_ptr as *mut c_void);
                    }
                    s
                };
                return Err(msg);
            }
            Ok(())
        }

        pub fn sync_docs(&self, docs: &[FtsSessionDoc]) -> Result<(), String> {
            self.exec_batch("BEGIN IMMEDIATE; DELETE FROM session_fts;")?;
            let sql = CString::new(
                "INSERT INTO session_fts (session_id, workspace, title, content, updated_at) VALUES (?1, ?2, ?3, ?4, ?5);",
            )
            .unwrap();
            for doc in docs {
                let mut stmt: *mut sqlite3_stmt = ptr::null_mut();
                let rc = unsafe {
                    sqlite3_prepare_v2(self.0, sql.as_ptr(), -1, &mut stmt, ptr::null_mut())
                };
                if rc != SQLITE_OK || stmt.is_null() {
                    let _ = self.exec_batch("ROLLBACK;");
                    return Err(format!("prepare insert failed rc={rc}"));
                }
                let bind_str = |idx: c_int, val: &str| unsafe {
                    let clean = val.replace('\0', " ");
                    sqlite3_bind_text(
                        stmt,
                        idx,
                        clean.as_ptr() as *const c_char,
                        clean.len() as c_int,
                        sqlite_transient(),
                    )
                };
                let _ = bind_str(1, &doc.session_id);
                let _ = bind_str(2, &doc.workspace);
                let _ = bind_str(3, &doc.title);
                let _ = bind_str(4, &doc.content);
                unsafe {
                    let _ = sqlite3_bind_int64(stmt, 5, doc.updated_at as i64);
                    let _ = sqlite3_step(stmt);
                    let _ = sqlite3_finalize(stmt);
                }
            }
            self.exec_batch("COMMIT;")
        }

        pub fn query(&self, fts_phrase: &str, limit: usize) -> Result<Vec<FtsSessionHit>, String> {
            let sql = CString::new(
                "SELECT session_id, workspace, title,
                        snippet(session_fts, 3, '«', '»', '…', 20),
                        updated_at,
                        bm25(session_fts, 0.0, 0.0, 5.0, 1.0) AS rank_score
                 FROM session_fts
                 WHERE session_fts MATCH ?1
                 ORDER BY rank_score ASC, updated_at DESC
                 LIMIT ?2;",
            )
            .unwrap();
            let mut stmt: *mut sqlite3_stmt = ptr::null_mut();
            let rc = unsafe {
                sqlite3_prepare_v2(self.0, sql.as_ptr(), -1, &mut stmt, ptr::null_mut())
            };
            if rc != SQLITE_OK || stmt.is_null() {
                return Err(format!("prepare query failed rc={rc}"));
            }
            let clean_q = fts_phrase.replace('\0', " ");
            unsafe {
                sqlite3_bind_text(
                    stmt,
                    1,
                    clean_q.as_ptr() as *const c_char,
                    clean_q.len() as c_int,
                    sqlite_transient(),
                );
                sqlite3_bind_int64(stmt, 2, limit.clamp(1, 100) as i64);
            }
            let mut hits = Vec::new();
            unsafe {
                while sqlite3_step(stmt) == SQLITE_ROW {
                    let col_str = |col: c_int| -> String {
                        let p = sqlite3_column_text(stmt, col);
                        if p.is_null() {
                            String::new()
                        } else {
                            CStr::from_ptr(p as *const c_char)
                                .to_string_lossy()
                                .into_owned()
                        }
                    };
                    let session_id = col_str(0);
                    let workspace = col_str(1);
                    let title = col_str(2);
                    let snippet = col_str(3);
                    let updated_at = sqlite3_column_int64(stmt, 4).max(0) as u64;
                    let bm25_neg = sqlite3_column_double(stmt, 5);
                    hits.push(FtsSessionHit {
                        session_id,
                        workspace,
                        title,
                        snippet,
                        updated_at,
                        score: -bm25_neg,
                    });
                }
                sqlite3_finalize(stmt);
            }
            Ok(hits)
        }
    }
}

pub struct SessionQueryFtsIndex {
    #[cfg(unix)]
    conn: Mutex<Option<unix_sqlite::SqliteConn>>,
    #[cfg(not(unix))]
    _placeholder: Mutex<()>,
}

impl SessionQueryFtsIndex {
    pub fn open(db_path: Option<&Path>) -> Self {
        #[cfg(unix)]
        {
            let conn = unix_sqlite::SqliteConn::open(db_path).ok();
            Self {
                conn: Mutex::new(conn),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = db_path;
            Self {
                _placeholder: Mutex::new(()),
            }
        }
    }

    pub fn search(
        &self,
        docs: &[FtsSessionDoc],
        raw_query: &str,
        limit: usize,
    ) -> Vec<FtsSessionHit> {
        let Some(phrase) = build_safe_fts5_phrase_query(raw_query) else {
            return Vec::new();
        };
        #[cfg(unix)]
        {
            let guard = self.conn.lock();
            if let Some(ref conn) = *guard {
                if conn.sync_docs(docs).is_ok() {
                    if let Ok(hits) = conn.query(&phrase, limit) {
                        if !hits.is_empty() {
                            return hits;
                        }
                    }
                }
            }
        }
        // Deterministic case-insensitive substring fallback (also catches partial identifier tokens).
        let needle = raw_query.trim().to_lowercase();
        let mut fallback: Vec<FtsSessionHit> = docs
            .iter()
            .filter_map(|d| {
                let title_l = d.title.to_lowercase();
                let body_l = d.content.to_lowercase();
                let title_matches = title_l.matches(&needle).count();
                let body_matches = body_l.matches(&needle).count();
                if title_matches == 0 && body_matches == 0 {
                    return None;
                }
                let snippet = if let Some(pos) = body_l.find(&needle) {
                    let start = d
                        .content
                        .char_indices()
                        .map(|(i, _)| i)
                        .take_while(|&i| i <= pos.saturating_sub(48))
                        .last()
                        .unwrap_or(0);
                    let snippet_str: String = d.content[start..].chars().take(160).collect();
                    snippet_str.replace('\n', " ")
                } else {
                    d.title.clone()
                };
                Some(FtsSessionHit {
                    session_id: d.session_id.clone(),
                    workspace: d.workspace.clone(),
                    title: d.title.clone(),
                    snippet,
                    updated_at: d.updated_at,
                    score: (title_matches * 5 + body_matches) as f64,
                })
            })
            .collect();
        fallback.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
        });
        fallback.truncate(limit.clamp(1, 100));
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_fts5_ranks_and_neutralizes_boolean_operator_injection() {
        let idx = SessionQueryFtsIndex::open(None);
        let docs = vec![
            FtsSessionDoc {
                session_id: "s1".into(),
                workspace: "/repo".into(),
                title: "Cordis Fiber Unload".into(),
                content: "Implemented guarded L-Unload dependent drain in Rust.".into(),
                updated_at: 100,
            },
            FtsSessionDoc {
                session_id: "s2".into(),
                workspace: "/repo".into(),
                title: "Grocery List".into(),
                content: "Apples and bananas.".into(),
                updated_at: 200,
            },
        ];
        // Normal phrase match:
        let hits = idx.search(&docs, "dependent drain", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "s1");
        assert!(hits[0].snippet.contains("dependent"));

        // Hostile FTS5 operator syntax (`OR * NEAR(`) must NOT match unrelated rows or error!
        let hostile = idx.search(&docs, "nonexistent\" OR Apples NEAR(*", 10);
        assert!(hostile.is_empty());
    }
}
