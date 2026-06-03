use serde::Deserialize;
use worker::D1Database;
use worker::d1::D1Type;

#[derive(Debug, Deserialize)]
pub(crate) struct AuditRow {
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub description: String,
    pub metadata: Option<String>,
}

pub(crate) async fn append_audit(
    db: &D1Database,
    event_id: &str,
    actor: &str,
    action: &str,
    target: &str,
    description: &str,
    metadata: Option<&str>,
) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO audit_log (event_id, actor, action, target, description, metadata) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    );
    stmt.bind_refs(&[
        D1Type::Text(event_id),
        D1Type::Text(actor),
        D1Type::Text(action),
        D1Type::Text(target),
        D1Type::Text(description),
        match metadata {
            Some(v) => D1Type::Text(v),
            None => D1Type::Null,
        },
    ])
    .map_err(|e| format!("D1 append_audit bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 append_audit run: {e:?}"))?;

    Ok(())
}

pub(crate) async fn get_audit_entries(
    db: &D1Database,
    event_id: &str,
    limit: usize,
) -> Result<Vec<AuditRow>, String> {
    let stmt = db.prepare(
        "SELECT timestamp, actor, action, target, description, metadata \
         FROM audit_log WHERE event_id = ?1 \
         ORDER BY timestamp DESC LIMIT ?2",
    );
    stmt.bind_refs(&[D1Type::Text(event_id), D1Type::Integer(limit as i32)])
        .map_err(|e| format!("D1 get_audit_entries bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("D1 get_audit_entries run: {e:?}"))?
        .results::<AuditRow>()
        .map_err(|e| format!("D1 get_audit_entries deserialize: {e:?}"))
}

pub(crate) async fn get_global_audit_entries(
    db: &D1Database,
    limit: usize,
) -> Result<Vec<AuditRow>, String> {
    let stmt = db.prepare(
        "SELECT timestamp, actor, action, target, description, metadata \
         FROM audit_log \
         ORDER BY timestamp DESC LIMIT ?1",
    );
    stmt.bind_refs(&[D1Type::Integer(limit as i32)])
        .map_err(|e| format!("D1 get_global_audit_entries bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("D1 get_global_audit_entries run: {e:?}"))?
        .results::<AuditRow>()
        .map_err(|e| format!("D1 get_global_audit_entries deserialize: {e:?}"))
}
