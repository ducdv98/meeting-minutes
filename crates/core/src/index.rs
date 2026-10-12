use std::{fs, path::Path};

use rusqlite::{params, Connection, ErrorCode};

use crate::{library::LibraryError, MeetingSummary};

const SCHEMA_VERSION: i64 = 1;

pub(crate) struct Index {
    connection: Connection,
    /// True when this open created the tables, so the index holds no Meetings yet.
    pub(crate) fresh: bool,
}

impl Index {
    pub(crate) fn open(path: &Path) -> Result<Self, LibraryError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        match Self::open_once(path) {
            Ok(index) => Ok(index),
            Err(LibraryError::Index(error)) if is_corrupt(&error) => {
                fs::remove_file(path)?;
                Self::open_once(path)
            }
            Err(error) => Err(error),
        }
    }

    fn open_once(path: &Path) -> Result<Self, LibraryError> {
        let connection = Connection::open(path)?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let fresh = version != SCHEMA_VERSION;
        if fresh {
            connection.execute_batch(
                "BEGIN;
                 DROP TABLE IF EXISTS meetings;
                 CREATE TABLE meetings (
                     id TEXT PRIMARY KEY,
                     title TEXT NOT NULL,
                     started_at TEXT NOT NULL,
                     started_at_ms INTEGER NOT NULL
                 );
                 PRAGMA user_version = 1;
                 COMMIT;",
            )?;
        }
        connection.prepare("SELECT id, title, started_at FROM meetings LIMIT 0")?;
        Ok(Self { connection, fresh })
    }

    pub(crate) fn insert(
        &self,
        meeting: &MeetingSummary,
        started_at_ms: i64,
    ) -> Result<(), LibraryError> {
        self.connection.execute(
            "INSERT OR REPLACE INTO meetings (id, title, started_at, started_at_ms) VALUES (?1, ?2, ?3, ?4)",
            params![meeting.id, meeting.title, meeting.started_at, started_at_ms],
        )?;
        Ok(())
    }

    pub(crate) fn replace(
        &mut self,
        meetings: &[(MeetingSummary, i64)],
    ) -> Result<(), LibraryError> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM meetings", [])?;
        for (meeting, started_at_ms) in meetings {
            transaction.execute(
                "INSERT INTO meetings (id, title, started_at, started_at_ms) VALUES (?1, ?2, ?3, ?4)",
                params![meeting.id, meeting.title, meeting.started_at, started_at_ms],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn list(&self) -> Result<Vec<MeetingSummary>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT id, title, started_at FROM meetings ORDER BY started_at_ms DESC, title ASC, id ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(MeetingSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                started_at: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn is_corrupt(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(failure, _)
        if matches!(failure.code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt))
}
