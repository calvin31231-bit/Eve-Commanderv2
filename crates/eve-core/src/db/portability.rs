//! Data portability: export all durable user data to a JSON bundle, and wipe it.
//!
//! Reinforces the local-first trust story (ROADMAP "Data portability & privacy":
//! one-click export / wipe). The export excludes secrets — refresh tokens live in
//! the OS keychain, never the DB — and the regenerable name cache. Wipe clears
//! the durable tables; tokens are cleared separately by the caller.

use sqlx::Row;

use crate::error::Result;

use super::Database;

impl Database {
    /// Export all durable user data as a JSON bundle (settings, characters,
    /// groups + memberships, snapshots, AI memory). Secrets and the regenerable
    /// name cache are intentionally omitted.
    pub async fn export_all(&self) -> Result<serde_json::Value> {
        let settings = sqlx::query("SELECT key, value FROM settings")
            .fetch_all(&self.app)
            .await?
            .iter()
            .map(|r| serde_json::json!({ "key": r.get::<String, _>("key"), "value": r.get::<String, _>("value") }))
            .collect::<Vec<_>>();

        let characters = sqlx::query(
            "SELECT id, name, corporation_id, alliance_id, scopes, active, added_at FROM characters",
        )
        .fetch_all(&self.app)
        .await?
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.get::<i64, _>("id"),
                "name": r.get::<String, _>("name"),
                "corporation_id": r.get::<Option<i64>, _>("corporation_id"),
                "alliance_id": r.get::<Option<i64>, _>("alliance_id"),
                "scopes": r.get::<String, _>("scopes"),
                "active": r.get::<i64, _>("active"),
                "added_at": r.get::<i64, _>("added_at"),
            })
        })
        .collect::<Vec<_>>();

        let groups = sqlx::query("SELECT id, name FROM character_groups")
            .fetch_all(&self.app)
            .await?
            .iter()
            .map(|r| serde_json::json!({ "id": r.get::<i64, _>("id"), "name": r.get::<String, _>("name") }))
            .collect::<Vec<_>>();

        let group_members = sqlx::query("SELECT group_id, character_id FROM character_group_members")
            .fetch_all(&self.app)
            .await?
            .iter()
            .map(|r| serde_json::json!({ "group_id": r.get::<i64, _>("group_id"), "character_id": r.get::<i64, _>("character_id") }))
            .collect::<Vec<_>>();

        let snapshots = sqlx::query("SELECT character_id, kind, taken_at, value FROM snapshots")
            .fetch_all(&self.app)
            .await?
            .iter()
            .map(|r| {
                serde_json::json!({
                    "character_id": r.get::<i64, _>("character_id"),
                    "kind": r.get::<String, _>("kind"),
                    "taken_at": r.get::<i64, _>("taken_at"),
                    "value": r.get::<f64, _>("value"),
                })
            })
            .collect::<Vec<_>>();

        let ai_memory = sqlx::query(
            "SELECT kind, title, body, salience, pinned, created_at, updated_at FROM ai_memory",
        )
        .fetch_all(&self.app)
        .await?
        .iter()
        .map(|r| {
            serde_json::json!({
                "kind": r.get::<String, _>("kind"),
                "title": r.get::<String, _>("title"),
                "body": r.get::<String, _>("body"),
                "salience": r.get::<f64, _>("salience"),
                "pinned": r.get::<i64, _>("pinned"),
                "created_at": r.get::<i64, _>("created_at"),
                "updated_at": r.get::<i64, _>("updated_at"),
            })
        })
        .collect::<Vec<_>>();

        let skill_plans = sqlx::query("SELECT name, body, created_at, updated_at FROM skill_plans")
            .fetch_all(&self.app)
            .await?
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.get::<String, _>("name"),
                    "body": r.get::<String, _>("body"),
                    "created_at": r.get::<i64, _>("created_at"),
                    "updated_at": r.get::<i64, _>("updated_at"),
                })
            })
            .collect::<Vec<_>>();

        let saved_fits = sqlx::query("SELECT name, ship, eft, created_at, updated_at FROM saved_fits")
            .fetch_all(&self.app)
            .await?
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.get::<String, _>("name"),
                    "ship": r.get::<String, _>("ship"),
                    "eft": r.get::<String, _>("eft"),
                    "created_at": r.get::<i64, _>("created_at"),
                    "updated_at": r.get::<i64, _>("updated_at"),
                })
            })
            .collect::<Vec<_>>();

        let implant_loadouts = sqlx::query("SELECT name, implant_ids, created_at, updated_at FROM implant_loadouts")
            .fetch_all(&self.app)
            .await?
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.get::<String, _>("name"),
                    "implant_ids": r.get::<String, _>("implant_ids"),
                    "created_at": r.get::<i64, _>("created_at"),
                    "updated_at": r.get::<i64, _>("updated_at"),
                })
            })
            .collect::<Vec<_>>();

        let abyss_runs = sqlx::query(
            "SELECT ran_at, tier, weather, ship, fit, duration_seconds, loot_value, survived, notes FROM abyss_runs",
        )
        .fetch_all(&self.app)
        .await?
        .iter()
        .map(|r| {
            serde_json::json!({
                "ran_at": r.get::<i64, _>("ran_at"),
                "tier": r.get::<i64, _>("tier"),
                "weather": r.get::<String, _>("weather"),
                "ship": r.get::<String, _>("ship"),
                "fit": r.get::<String, _>("fit"),
                "duration_seconds": r.get::<i64, _>("duration_seconds"),
                "loot_value": r.get::<f64, _>("loot_value"),
                "survived": r.get::<i64, _>("survived"),
                "notes": r.get::<String, _>("notes"),
            })
        })
        .collect::<Vec<_>>();

        let srp_claims = sqlx::query(
            "SELECT submitted_at, pilot, ship, loss_value, location, killmail_url, notes, status, payout, reviewer_note, decided_at FROM srp_claims",
        )
        .fetch_all(&self.app)
        .await?
        .iter()
        .map(|r| {
            serde_json::json!({
                "submitted_at": r.get::<i64, _>("submitted_at"),
                "pilot": r.get::<String, _>("pilot"),
                "ship": r.get::<String, _>("ship"),
                "loss_value": r.get::<f64, _>("loss_value"),
                "location": r.get::<String, _>("location"),
                "killmail_url": r.get::<String, _>("killmail_url"),
                "notes": r.get::<String, _>("notes"),
                "status": r.get::<String, _>("status"),
                "payout": r.get::<f64, _>("payout"),
                "reviewer_note": r.get::<String, _>("reviewer_note"),
                "decided_at": r.get::<Option<i64>, _>("decided_at"),
            })
        })
        .collect::<Vec<_>>();

        let recruits = sqlx::query(
            "SELECT applied_at, name, source, notes, status, recruiter, reviewer_note, decided_at FROM recruits",
        )
        .fetch_all(&self.app)
        .await?
        .iter()
        .map(|r| {
            serde_json::json!({
                "applied_at": r.get::<i64, _>("applied_at"),
                "name": r.get::<String, _>("name"),
                "source": r.get::<String, _>("source"),
                "notes": r.get::<String, _>("notes"),
                "status": r.get::<String, _>("status"),
                "recruiter": r.get::<String, _>("recruiter"),
                "reviewer_note": r.get::<String, _>("reviewer_note"),
                "decided_at": r.get::<Option<i64>, _>("decided_at"),
            })
        })
        .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "format": "eve-commander-export",
            "version": 1,
            "settings": settings,
            "characters": characters,
            "groups": groups,
            "group_members": group_members,
            "snapshots": snapshots,
            "ai_memory": ai_memory,
            "skill_plans": skill_plans,
            "saved_fits": saved_fits,
            "implant_loadouts": implant_loadouts,
            "abyss_runs": abyss_runs,
            "srp_claims": srp_claims,
            "recruits": recruits,
        }))
    }

    /// Delete all durable user data (every table). The OS-keychain tokens are not
    /// touched here — the caller clears those separately. Irreversible.
    pub async fn wipe_all(&self) -> Result<()> {
        for table in [
            "character_group_members",
            "character_groups",
            "snapshots",
            "ai_memory",
            "skill_plans",
            "saved_fits",
            "implant_loadouts",
            "abyss_runs",
            "srp_claims",
            "recruits",
            "names",
            "settings",
            "characters",
        ] {
            sqlx::query(&format!("DELETE FROM {table}")).execute(&self.app).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn export_then_wipe() {
        let db = Database::open_in_memory().await.unwrap();
        db.set_setting("intensity", "Aggressive").await.unwrap();
        db.add_memory("goal", "Carrier", "Train toward a Nyx", 0.8, 100).await.unwrap();

        let bundle = db.export_all().await.unwrap();
        assert_eq!(bundle["format"], "eve-commander-export");
        assert_eq!(bundle["settings"].as_array().unwrap().len(), 1);
        assert_eq!(bundle["ai_memory"].as_array().unwrap().len(), 1);

        db.wipe_all().await.unwrap();
        let after = db.export_all().await.unwrap();
        assert!(after["settings"].as_array().unwrap().is_empty());
        assert!(after["ai_memory"].as_array().unwrap().is_empty());
    }
}
