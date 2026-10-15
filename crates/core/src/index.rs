use std::{fs, path::Path};

use rusqlite::{params, Connection, ErrorCode};

use crate::{
    library::LibraryError, participant::Participants, transcript::Transcript, MeetingSummary,
    SearchHit,
};

const SCHEMA_VERSION: i64 = 5;

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
                 DROP TABLE IF EXISTS segments;
                 CREATE TABLE meetings (
                     id TEXT PRIMARY KEY,
                     title TEXT NOT NULL,
                     started_at TEXT NOT NULL,
                     started_at_ms INTEGER NOT NULL,
                     duration_ms INTEGER NOT NULL
                 );
                 CREATE VIRTUAL TABLE segments USING fts5(
                     text, participant, folded,
                     meeting_id UNINDEXED, revision UNINDEXED, segment_id UNINDEXED, start_ms UNINDEXED, end_ms UNINDEXED,
                     tokenize = 'unicode61 remove_diacritics 2'
                 );
                 PRAGMA user_version = 5;
                 COMMIT;",
            )?;
        }
        connection.prepare("SELECT id, title, started_at, duration_ms FROM meetings LIMIT 0")?;
        connection.prepare("SELECT text, participant, meeting_id FROM segments LIMIT 0")?;
        Ok(Self { connection, fresh })
    }

    pub(crate) fn insert(
        &self,
        meeting: &MeetingSummary,
        started_at_ms: i64,
    ) -> Result<(), LibraryError> {
        self.connection.execute(
            "INSERT OR REPLACE INTO meetings (id, title, started_at, started_at_ms, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![meeting.id, meeting.title, meeting.started_at, started_at_ms, meeting.duration_ms as i64],
        )?;
        Ok(())
    }

    pub(crate) fn replace(
        &mut self,
        meetings: &[(MeetingSummary, i64, Option<Transcript>, Participants)],
    ) -> Result<(), LibraryError> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM meetings", [])?;
        transaction.execute("DELETE FROM segments", [])?;
        for (meeting, started_at_ms, transcript, participants) in meetings {
            transaction.execute(
                "INSERT INTO meetings (id, title, started_at, started_at_ms, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![meeting.id, meeting.title, meeting.started_at, started_at_ms, meeting.duration_ms as i64],
            )?;
            if let Some(transcript) = transcript {
                insert_segments(&transaction, &meeting.id, transcript, participants)?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn set_transcript(
        &self,
        meeting_id: &str,
        transcript: Option<&Transcript>,
        participants: &Participants,
    ) -> Result<(), LibraryError> {
        // Library re-indexes from `&self` paths (Participant changes); the Library's own lock serializes writers.
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute("DELETE FROM segments WHERE meeting_id = ?1", [meeting_id])?;
        if let Some(transcript) = transcript {
            insert_segments(&transaction, meeting_id, transcript, participants)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn remove_meeting(&mut self, meeting_id: &str) -> Result<(), LibraryError> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM segments WHERE meeting_id = ?1", [meeting_id])?;
        transaction.execute("DELETE FROM meetings WHERE id = ?1", [meeting_id])?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, LibraryError> {
        let Some(query) = fts_query(&crate::search_folding::fold(query)) else {
            return Ok(Vec::new());
        };
        let mut statement = self.connection.prepare(
            "WITH ranked AS (SELECT s.rowid AS segment_rowid, s.meeting_id, rank AS score, \
             ROW_NUMBER() OVER (PARTITION BY s.meeting_id ORDER BY rank, s.start_ms) AS hit_number \
             FROM segments s WHERE segments MATCH ?1) \
             SELECT s.meeting_id, m.title, m.started_at, s.segment_id, s.start_ms, s.end_ms, \
             snippet(segments, 0, char(2), char(3), '…', 12), s.revision \
             FROM segments s JOIN ranked r ON r.segment_rowid = s.rowid \
             JOIN meetings m ON m.id = s.meeting_id \
             WHERE segments MATCH ?1 AND r.hit_number <= 5 \
             ORDER BY r.score, m.started_at_ms DESC, s.start_ms LIMIT ?2",
        )?;
        let rows = statement.query_map(params![query, limit as i64], |row| {
            Ok(SearchHit {
                meeting_id: row.get(0)?,
                title: row.get(1)?,
                started_at: row.get(2)?,
                segment_id: row.get(3)?,
                start_ms: row.get(4)?,
                end_ms: row.get(5)?,
                snippet: row.get(6)?,
                revision: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub(crate) fn list(&self) -> Result<Vec<MeetingSummary>, LibraryError> {
        let mut statement = self.connection.prepare(
            "SELECT id, title, started_at, duration_ms FROM meetings ORDER BY started_at_ms DESC, title ASC, id ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(MeetingSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                started_at: row.get(2)?,
                duration_ms: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn insert_segments(
    transaction: &rusqlite::Transaction<'_>,
    meeting_id: &str,
    transcript: &Transcript,
    participants: &Participants,
) -> Result<(), LibraryError> {
    for segment in &transcript.segments {
        transaction.execute(
            "INSERT INTO segments (text, participant, folded, meeting_id, revision, segment_id, start_ms, end_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![segment.text, participants.name(segment, transcript.revision), crate::search_folding::fold(&format!("{} {}", segment.text, participants.name(segment, transcript.revision))), meeting_id, transcript.revision, segment.id, segment.start_ms, segment.end_ms],
        )?;
    }
    Ok(())
}

fn fts_query(input: &str) -> Option<String> {
    let pieces: Vec<_> = input
        .split(|character: char| !character.is_alphanumeric())
        .filter(|piece| !piece.is_empty())
        .map(|piece| format!("\"{piece}\"*"))
        .collect();
    if pieces.is_empty() {
        None
    } else {
        Some(pieces.join(" "))
    }
}

fn is_corrupt(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(failure, _)
        if matches!(failure.code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt))
}
