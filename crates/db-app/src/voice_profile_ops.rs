use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceProfile {
    pub id: String,
    pub human_id: String,
    pub embedding: Vec<f32>,
    pub dim: i64,
    pub model: String,
    pub sample_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(sqlx::FromRow)]
struct VoiceProfileRow {
    id: String,
    human_id: String,
    embedding: Vec<u8>,
    dim: i64,
    model: String,
    sample_count: i64,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

fn encode_embedding(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .copied()
        .flat_map(f32::to_le_bytes)
        .collect()
}

fn decode_embedding(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err(format!(
            "embedding blob length {} is not a multiple of 4",
            bytes.len()
        ));
    }

    let mut values = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let array: [u8; 4] = chunk.try_into().expect("chunk length is exactly 4");
        values.push(f32::from_le_bytes(array));
    }
    Ok(values)
}

fn decode_error(message: String) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message,
    )))
}

pub async fn list_voice_profiles(pool: &SqlitePool) -> Result<Vec<VoiceProfile>, sqlx::Error> {
    let rows: Vec<VoiceProfileRow> = sqlx::query_as(
        "SELECT id, human_id, embedding, dim, model, sample_count, created_at, updated_at, deleted_at
         FROM voice_profiles
         WHERE deleted_at IS NULL
         ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await?;

    let mut profiles = Vec::with_capacity(rows.len());
    for row in rows {
        let embedding = decode_embedding(&row.embedding).map_err(decode_error)?;
        profiles.push(VoiceProfile {
            id: row.id,
            human_id: row.human_id,
            embedding,
            dim: row.dim,
            model: row.model,
            sample_count: row.sample_count,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        });
    }
    Ok(profiles)
}

pub async fn upsert_voice_profile(
    pool: &SqlitePool,
    profile: &VoiceProfile,
) -> Result<(), sqlx::Error> {
    let embedding_blob = encode_embedding(&profile.embedding);

    sqlx::query(
        "INSERT INTO voice_profiles \
         (id, human_id, embedding, dim, model, sample_count, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           human_id = excluded.human_id, \
           embedding = excluded.embedding, \
           dim = excluded.dim, \
           model = excluded.model, \
           sample_count = excluded.sample_count, \
           updated_at = excluded.updated_at, \
           deleted_at = NULL",
    )
    .bind(&profile.id)
    .bind(&profile.human_id)
    .bind(&embedding_blob)
    .bind(profile.embedding.len() as i64)
    .bind(&profile.model)
    .bind(profile.sample_count)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_voice_profile(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE voice_profiles
         SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use hypr_db_core::Db;

    use super::*;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        crate::prepare_schema(&db).await.unwrap();
        db
    }

    #[test]
    fn embedding_blob_roundtrip_is_bit_exact() {
        let embedding: Vec<f32> = vec![0.0, -1.5, 1.2345678, f32::MIN, f32::MAX, 1e-37];
        let blob = encode_embedding(&embedding);
        let decoded = decode_embedding(&blob).unwrap();

        assert_eq!(blob.len(), embedding.len() * 4);
        assert_eq!(decoded, embedding);
    }

    #[tokio::test]
    async fn voice_profile_roundtrip() {
        let db = test_db().await;

        sqlx::query("INSERT INTO humans (id, name) VALUES ('human-1', 'Alice')")
            .execute(db.pool())
            .await
            .unwrap();

        let profile = VoiceProfile {
            id: "vp-1".to_string(),
            human_id: "human-1".to_string(),
            embedding: vec![0.1, -0.2, 0.3],
            dim: 3,
            model: "test-model".to_string(),
            sample_count: 5,
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
        };

        upsert_voice_profile(db.pool(), &profile).await.unwrap();

        let profiles = list_voice_profiles(db.pool()).await.unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, "vp-1");
        assert_eq!(profiles[0].human_id, "human-1");
        assert_eq!(profiles[0].embedding, vec![0.1, -0.2, 0.3]);
        assert_eq!(profiles[0].dim, 3);
        assert_eq!(profiles[0].model, "test-model");
        assert_eq!(profiles[0].sample_count, 5);

        delete_voice_profile(db.pool(), "vp-1").await.unwrap();
        assert!(list_voice_profiles(db.pool()).await.unwrap().is_empty());
    }
}
