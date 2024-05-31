use crate::proto::{EmptyReply, FindUniqueRecordRequest, RecordReply, UpsertRecordRequest};
use anyhow::Result;
use tokio_rusqlite::params;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tokio_rusqlite::Connection;

static HEALTHY_AGE: Duration = Duration::minutes(7);

#[derive(Debug)]
pub struct RecordRepository {
    pub conn: Arc<Connection>,
}

impl RecordRepository {
    pub async fn find_unique(&self, request: FindUniqueRecordRequest) -> Result<RecordReply> {
        let name = request.name;
        let r#type = request.r#type.clone();
        let record = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    r#"
SELECT name, type, data, ttl, created_at, updated_at
FROM records
WHERE name = ?1
  AND type = ?2"#,
                )?;
                Ok(stmt.query_row([name, r#type], |row| {
                    let created_at: OffsetDateTime = row.get(4)?;
                    let updated_at: OffsetDateTime = row.get(5)?;
                    Ok(RecordReply {
                        name: row.get(0)?,
                        r#type: row.get(1)?,
                        data: row.get(2)?,
                        ttl: row.get(3)?,
                        created_at: created_at.unix_timestamp(),
                        updated_at: updated_at.unix_timestamp(),
                        healthy: true,
                    })
                }))
            })
            .await?;
        Ok(record?)
    }

    pub async fn upsert(&self, request: UpsertRecordRequest) -> Result<RecordReply> {
        let name = request.name.clone();
        let name2 = name.clone();
        let r#type = request.r#type;
        let r#type2 = r#type.clone();
        let data = request.value;
        let ttl = request.ttl;
        let now = OffsetDateTime::now_utc();

        self.conn
            .call(move |conn| {
                conn.execute(
                    r#"
INSERT INTO records (name,
                     type,
                     data,
                     ttl,
                     created_at,
                     updated_at)
VALUES (?1,
        ?2,
        ?3,
        ?4,
        ?5,
        ?6)
ON CONFLICT(name, type)
    DO UPDATE SET data       = excluded.data,
                  ttl        = excluded.ttl,
                  updated_at = excluded.updated_at"#,
                    params![name, r#type, data, ttl, now, now],
                )?;
                Ok(())
            })
            .await?;

        self.find_unique(FindUniqueRecordRequest {
            name: name2,
            r#type: r#type2,
        })
        .await
    }

    pub async fn list(&self) -> Result<Vec<RecordReply>> {
        let records = self
            .conn
            .call(|conn| {
                let mut stmt = conn
                    .prepare("SELECT name, type, data, ttl, created_at, updated_at FROM records")?;
                let records = stmt
                    .query_map([], |row| {
                        let created_at: OffsetDateTime = row.get(4)?;
                        let updated_at: OffsetDateTime = row.get(5)?;
                        Ok(RecordReply {
                            name: row.get(0)?,
                            r#type: row.get(1)?,
                            data: row.get(2)?,
                            ttl: row.get(3)?,
                            created_at: created_at.unix_timestamp(),
                            updated_at: updated_at.unix_timestamp(),
                            healthy: OffsetDateTime::now_utc() - updated_at <= HEALTHY_AGE,
                        })
                    })?
                    .collect::<Result<Vec<RecordReply>, _>>()?;
                Ok(records)
            })
            .await?;
        Ok(records)
    }

    pub async fn delete(&self, request: FindUniqueRecordRequest) -> Result<()> {
        let name = request.name;
        let r#type = request.r#type;
        self.conn
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM records WHERE name = ?1 AND type = ?2",
                    params![name, r#type],
                )?;
                Ok(EmptyReply {})
            })
            .await?;
        Ok(())
    }
}
