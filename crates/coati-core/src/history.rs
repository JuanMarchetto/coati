use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct HistoryRepo {
    conn: Connection,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub model: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub model: String,
    pub created_at: i64,
}

impl HistoryRepo {
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from(".local/share"))
            .join("coati/history.db")
    }

    pub fn open_default() -> anyhow::Result<Self> {
        let path = Self::default_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::open(&path)
    }

    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS conversations (
              id TEXT PRIMARY KEY,
              title TEXT NOT NULL,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              model TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
              id TEXT PRIMARY KEY,
              conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
              role TEXT NOT NULL,
              content TEXT NOT NULL,
              model TEXT NOT NULL,
              created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_messages_conv_time
              ON messages(conversation_id, created_at);
            "#,
        )?;
        Ok(Self { conn })
    }

    pub fn create_conversation(&self, title: &str, model: &str) -> anyhow::Result<Conversation> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        self.conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at, model) VALUES (?, ?, ?, ?, ?)",
            params![id, title, now, now, model],
        )?;
        Ok(Conversation {
            id,
            title: title.into(),
            created_at: now,
            updated_at: now,
            model: model.into(),
        })
    }

    pub fn append_message(
        &self,
        conv_id: &str,
        role: &str,
        content: &str,
        model: &str,
    ) -> anyhow::Result<Message> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        self.conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, model, created_at) VALUES (?, ?, ?, ?, ?, ?)",
            params![id, conv_id, role, content, model, now],
        )?;
        self.conn.execute(
            "UPDATE conversations SET updated_at = ? WHERE id = ?",
            params![now, conv_id],
        )?;
        Ok(Message {
            id,
            conversation_id: conv_id.into(),
            role: role.into(),
            content: content.into(),
            model: model.into(),
            created_at: now,
        })
    }

    pub fn messages(&self, conv_id: &str) -> anyhow::Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, conversation_id, role, content, model, created_at
             FROM messages WHERE conversation_id = ? ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![conv_id], |r| {
            Ok(Message {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                model: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn list_conversations(&self, limit: u32) -> anyhow::Result<Vec<Conversation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, created_at, updated_at, model FROM conversations
             ORDER BY updated_at DESC LIMIT ?",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(Conversation {
                id: r.get(0)?,
                title: r.get(1)?,
                created_at: r.get(2)?,
                updated_at: r.get(3)?,
                model: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn opens_fresh_db_and_creates_schema() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("h.db");
        let _repo = HistoryRepo::open(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn round_trips_conversation_and_messages() {
        let dir = TempDir::new().unwrap();
        let repo = HistoryRepo::open(&dir.path().join("h.db")).unwrap();
        let conv = repo.create_conversation("test", "gemma4").unwrap();
        repo.append_message(&conv.id, "user", "hi", "gemma4")
            .unwrap();
        repo.append_message(&conv.id, "assistant", "hello", "gemma4")
            .unwrap();
        let ms = repo.messages(&conv.id).unwrap();
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[0].role, "user");
        assert_eq!(ms[1].content, "hello");
    }

    #[test]
    fn list_orders_by_updated_desc() {
        let dir = TempDir::new().unwrap();
        let repo = HistoryRepo::open(&dir.path().join("h.db")).unwrap();
        let a = repo.create_conversation("a", "gemma4").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        let b = repo.create_conversation("b", "gemma4").unwrap();
        let list = repo.list_conversations(10).unwrap();
        assert_eq!(list[0].id, b.id);
        assert_eq!(list[1].id, a.id);
    }
}
