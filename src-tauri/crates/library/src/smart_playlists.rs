//! Smart playlist storage and single-rule evaluation engine.
//!
//! v1: single-rule only. Each playlist is stored in `smart_playlists`
//! with `field`, `operator`, `value`, `sort_field`, `sort_dir`, `limit_n`.
//! Evaluation runs a parametrized SQL query against the local SQLite cache.

use rusqlite::params;
use sinfonic_domain::{
    SmartPlaylist, SmartPlaylistId, SmartPlaylistRule,
    SmartPlaylistRuleField, SmartPlaylistRuleOperator, SmartPlaylistSortDirection,
    SmartPlaylistSortField, Track,
};

use crate::error::{LibraryError, LibraryResult};
use crate::store::Store;

// ─── SQL generation ──────────────────────────────────────────────

fn column_name(field: SmartPlaylistRuleField) -> &'static str {
    match field {
        SmartPlaylistRuleField::Title => "title",
        SmartPlaylistRuleField::Artist => "artist",
        SmartPlaylistRuleField::Album => "album",
        SmartPlaylistRuleField::Genre => "genre",
        SmartPlaylistRuleField::DurationSeconds => "duration_seconds",
        SmartPlaylistRuleField::TrackNumber => "track_number",
        SmartPlaylistRuleField::Year => "year",
        SmartPlaylistRuleField::Favorite => "favorite",
        SmartPlaylistRuleField::PlayCount => "play_count",
    }
}

fn operator_sql(op: SmartPlaylistRuleOperator) -> &'static str {
    match op {
        SmartPlaylistRuleOperator::Contains => "LIKE",
        SmartPlaylistRuleOperator::StartsWith => "LIKE",
        SmartPlaylistRuleOperator::EndsWith => "LIKE",
        SmartPlaylistRuleOperator::Equals => "=",
        SmartPlaylistRuleOperator::LessThan => "<",
        SmartPlaylistRuleOperator::GreaterThan => ">",
        SmartPlaylistRuleOperator::NotContains => "NOT LIKE",
        SmartPlaylistRuleOperator::NotEquals => "<>",
    }
}

fn bind_value(op: SmartPlaylistRuleOperator, value: &str) -> String {
    let escaped = value.replace('%', "%\\%").replace('_', "\\_");
    match op {
        SmartPlaylistRuleOperator::Contains => format!("%{escaped}%"),
        SmartPlaylistRuleOperator::StartsWith => format!("{escaped}%"),
        SmartPlaylistRuleOperator::EndsWith => format!("%{escaped}"),
        _ => escaped,
    }
}

fn sort_column(field: SmartPlaylistSortField) -> &'static str {
    match field {
        SmartPlaylistSortField::Title => "title COLLATE NOCASE",
        SmartPlaylistSortField::Artist => "artist COLLATE NOCASE",
        SmartPlaylistSortField::Album => "album COLLATE NOCASE",
        SmartPlaylistSortField::DurationSeconds => "duration_seconds",
        SmartPlaylistSortField::Year => "year",
        SmartPlaylistSortField::Random => "RANDOM()",
        SmartPlaylistSortField::DateAdded => "rowid",
    }
}

fn sort_direction_sql(dir: SmartPlaylistSortDirection) -> &'static str {
    match dir {
        SmartPlaylistSortDirection::Asc => "ASC",
        SmartPlaylistSortDirection::Desc => "DESC",
    }
}

fn build_where(rule: &SmartPlaylistRule) -> (String, String) {
    let col = column_name(rule.field);
    let op_sql = operator_sql(rule.operator);
    let bind = bind_value(rule.operator, &rule.value);
    let clause = format!("{col} {op_sql} ?");
    (clause, bind)
}

fn build_order_by(sort_field: SmartPlaylistSortField, sort_dir: SmartPlaylistSortDirection) -> String {
    let col = sort_column(sort_field);
    let dir = sort_direction_sql(sort_dir);
    if matches!(sort_field, SmartPlaylistSortField::Random) {
        "ORDER BY RANDOM()".to_string()
    } else {
        format!("ORDER BY {col} {dir}")
    }
}

/// Builds a full SELECT query for evaluating a smart playlist against
/// `tracks` joined with `album_genres`.
fn build_evaluate_query(
    rule: &SmartPlaylistRule,
    sort_field: SmartPlaylistSortField,
    sort_dir: SmartPlaylistSortDirection,
    limit: u16,
) -> (String, String) {
    let (where_clause, bind) = build_where(rule);
    let order_by = build_order_by(sort_field, sort_dir);

        let query = if matches!(rule.field, SmartPlaylistRuleField::Genre) {
        format!(
            "SELECT t.track_id, t.album_id, t.title, t.artist, t.artist_id, t.album, \
             t.duration_seconds, t.track_number, t.disc_number, t.favorite, t.year \
             FROM tracks t \
             JOIN album_genres ag ON ag.server_id = t.server_id AND ag.album_id = t.album_id \
             WHERE t.server_id = ? AND {where_clause} \
             {order_by} \
             LIMIT {limit}"
        )
    } else {
        format!(
            "SELECT track_id, album_id, title, artist, artist_id, album, \
             duration_seconds, track_number, disc_number, favorite, year \
             FROM tracks \
             WHERE server_id = ? AND {where_clause} \
             {order_by} \
             LIMIT {limit}"
        )
    };

    (query, bind)
}

// ─── Store methods ───────────────────────────────────────────────

impl Store {
    /// Persists (upserts) a smart playlist.
    pub fn replace_smart_playlists(
        &self,
        server_id: &sinfonic_domain::ServerId,
        playlists: &[SmartPlaylist],
    ) -> LibraryResult<()> {
        let conn = self.connection()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let mut stmt = conn.prepare(
            "INSERT INTO smart_playlists \
             (server_id, sp_id, name, field, operator, value, sort_field, sort_dir, limit_n, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
             ON CONFLICT(server_id, sp_id) DO UPDATE SET \
             name = excluded.name, field = excluded.field, operator = excluded.operator, \
             value = excluded.value, sort_field = excluded.sort_field, \
             sort_dir = excluded.sort_dir, limit_n = excluded.limit_n, updated_at = excluded.updated_at",
        )?;

        for sp in playlists {
            let field = serde_json::to_string(&sp.rule.field).unwrap();
            let operator = serde_json::to_string(&sp.rule.operator).unwrap();
            let sort_field = serde_json::to_string(&sp.sort_field).unwrap();
            let sort_dir = serde_json::to_string(&sp.sort_dir).unwrap();

            stmt.execute(params![
                server_id.as_str(),
                sp.id.as_str(),
                sp.name,
                field,
                operator,
                sp.rule.value,
                sort_field,
                sort_dir,
                sp.limit_n,
                now,
                now,
            ])?;
        }
        Ok(())
    }

    /// Lists all smart playlists for a server.
    pub fn list_smart_playlists(
        &self,
        server_id: &sinfonic_domain::ServerId,
    ) -> LibraryResult<Vec<SmartPlaylist>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT sp_id, name, field, operator, value, sort_field, sort_dir, limit_n \
             FROM smart_playlists WHERE server_id = ?1",
        )?;

        let rows = stmt.query_map([server_id.as_str()], |r| {
            let field_raw: String = r.get(2)?;
            let operator_raw: String = r.get(3)?;
            let sort_field_raw: String = r.get(5)?;
            let sort_dir_raw: String = r.get(6)?;

            Ok((
                r.get::<_, String>(0)?, // sp_id
                r.get::<_, String>(1)?, // name
                field_raw,
                operator_raw,
                r.get::<_, String>(4)?, // value
                sort_field_raw,
                sort_dir_raw,
                r.get::<_, i64>(7)?, // limit_n
            ))
        })?;

        let mut playlists = Vec::new();
        for row in rows {
            let (sp_id, name, field_raw, operator_raw, value, sort_field_raw, sort_dir_raw, limit_n) =
                row.map_err(|e| LibraryError::Validation(e.to_string()))?;

            let field: SmartPlaylistRuleField =
                serde_json::from_str(&field_raw).map_err(|e| LibraryError::Validation(e.to_string()))?;
            let operator: SmartPlaylistRuleOperator =
                serde_json::from_str(&operator_raw).map_err(|e| LibraryError::Validation(e.to_string()))?;
            let sort_field: SmartPlaylistSortField =
                serde_json::from_str(&sort_field_raw).map_err(|e| LibraryError::Validation(e.to_string()))?;
            let sort_dir: SmartPlaylistSortDirection =
                serde_json::from_str(&sort_dir_raw).map_err(|e| LibraryError::Validation(e.to_string()))?;

            playlists.push(SmartPlaylist {
                id: SmartPlaylistId::new(sp_id),
                name,
                rule: SmartPlaylistRule { field, operator, value },
                sort_field,
                sort_dir,
                limit_n: limit_n as u16,
            });
        }

        Ok(playlists)
    }

    /// Evaluates a smart playlist and returns matching tracks.
    pub fn evaluate_smart_playlist(
        &self,
        server_id: &sinfonic_domain::ServerId,
        sp: &SmartPlaylist,
    ) -> LibraryResult<Vec<Track>> {
        let conn = self.connection()?;
        let (query, bind) =
            build_evaluate_query(&sp.rule, sp.sort_field, sp.sort_dir, sp.limit_n);

        let mut stmt = conn.prepare(&query).map_err(|e| LibraryError::Validation(e.to_string()))?;

        let tracks = stmt
            .query_map(params![server_id.as_str(), bind], |r| {
                Ok(Track {
                    id: sinfonic_domain::TrackId::new(r.get::<_, String>(0)?),
                    album_id: sinfonic_domain::AlbumId::new(r.get::<_, String>(1)?),
                    title: r.get(2)?,
                    artist: r.get(3)?,
                    artist_id: r.get::<_, Option<String>>(4)?
                        .map(sinfonic_domain::ArtistId::new),
                    album: r.get(5)?,
                    duration_seconds: r.get::<_, i64>(6)? as u32,
                    track_number: r.get::<_, i64>(7)? as u16,
                    disc_number: r.get::<_, i64>(8)? as u16,
                    favorite: r.get::<_, i64>(9)? != 0,
                    image_ref: None,
                })
            })
            .map_err(|e| LibraryError::Validation(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| LibraryError::Validation(e.to_string()))?;

        Ok(tracks)
    }

    /// Deletes a smart playlist by ID.
    pub fn delete_smart_playlist(
        &self,
        server_id: &sinfonic_domain::ServerId,
        sp_id: &SmartPlaylistId,
    ) -> LibraryResult<()> {
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM smart_playlists WHERE server_id = ?1 AND sp_id = ?2",
            params![server_id.as_str(), sp_id.as_str()],
        )?;
        Ok(())
    }
}
