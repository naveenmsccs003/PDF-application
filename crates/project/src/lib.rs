//! Project/Security domain model (Section 7 layering: Domain "Project,
//! Security"). Persists through `mds_db`'s `user`/`project`/`project_member`
//! tables.
//!
//! Covers COLLAB-01 (shared project access), part of SEC-03 (access
//! control storage — membership + a check, not enforcement, since there's
//! no app layer yet to enforce anything against), and DOC-06 (shared
//! project membership, which is really this same membership model viewed
//! from the Document side). Role is stored as free text, not a fixed enum
//! — `docs/features/FEATURE_REGISTRY.md` still has the collaboration
//! model and exact role set marked "not confirmed" by Naveen, so this
//! doesn't invent role semantics (permission levels, what an "editor" can
//! and can't do) beyond what the schema already commits to (a project has
//! members, each with a role name).
//!
//! COLLAB-02 (async update visibility) isn't separate code: every query
//! function here (and in `document`/`markup`/`measurement`/`takeoff`)
//! reads current database state on each call, so one member's saved
//! change is simply what the next member's query returns — that *is*
//! async shared visibility, given there's no real-time sync layer
//! (COLLAB-03, explicitly BACKLOG). Nothing to add for it here.

use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("user {0} not found")]
    UserNotFound(String),
    #[error("project {0} not found")]
    ProjectNotFound(String),
    #[error("user {user_id} is not a member of project {project_id}")]
    NotAMember { project_id: String, user_id: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub created_by: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectMemberRecord {
    pub id: String,
    pub project_id: String,
    pub user_id: String,
    pub role: String,
}

pub fn create_user(conn: &Connection, email: &str, display_name: &str) -> Result<UserRecord, ProjectError> {
    let id = mds_db::new_uuid();
    conn.execute(
        "INSERT INTO user (id, email, display_name) VALUES (?1, ?2, ?3)",
        params![id, email, display_name],
    )?;
    Ok(UserRecord {
        id,
        email: email.to_string(),
        display_name: display_name.to_string(),
    })
}

pub fn get_user(conn: &Connection, id: &str) -> Result<UserRecord, ProjectError> {
    conn.query_row(
        "SELECT id, email, display_name FROM user WHERE id = ?1",
        params![id],
        |row| {
            Ok(UserRecord {
                id: row.get(0)?,
                email: row.get(1)?,
                display_name: row.get(2)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| ProjectError::UserNotFound(id.to_string()))
}

/// Convenience lookup, not auth — there's no password/session concept
/// anywhere in this schema yet (SEC-01/02 are still open, deliberately
/// deferred design decisions, see `docs/features/FEATURE_REGISTRY.md`).
/// Lets a caller identify a returning user by email instead of tripping
/// the `user.email` `UNIQUE` constraint by calling `create_user` again.
pub fn get_user_by_email(conn: &Connection, email: &str) -> Result<UserRecord, ProjectError> {
    conn.query_row(
        "SELECT id, email, display_name FROM user WHERE email = ?1",
        params![email],
        |row| {
            Ok(UserRecord {
                id: row.get(0)?,
                email: row.get(1)?,
                display_name: row.get(2)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| ProjectError::UserNotFound(email.to_string()))
}

/// COLLAB-01: creates a project and, atomically, adds `created_by` as its
/// first member with role `"owner"` — a project with no members at all
/// isn't a useful starting state for a "shared project" feature.
pub fn create_project(
    conn: &mut Connection,
    name: &str,
    created_by: &str,
) -> Result<ProjectRecord, ProjectError> {
    let tx = conn.transaction()?;
    let project_id = mds_db::new_uuid();
    tx.execute(
        "INSERT INTO project (id, name, created_by) VALUES (?1, ?2, ?3)",
        params![project_id, name, created_by],
    )?;
    tx.execute(
        "INSERT INTO project_member (id, project_id, user_id, role) VALUES (?1, ?2, ?3, 'owner')",
        params![mds_db::new_uuid(), project_id, created_by],
    )?;
    tx.commit()?;
    Ok(ProjectRecord {
        id: project_id,
        name: name.to_string(),
        created_by: created_by.to_string(),
    })
}

pub fn get_project(conn: &Connection, id: &str) -> Result<ProjectRecord, ProjectError> {
    conn.query_row(
        "SELECT id, name, created_by FROM project WHERE id = ?1",
        params![id],
        |row| {
            Ok(ProjectRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                created_by: row.get(2)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| ProjectError::ProjectNotFound(id.to_string()))
}

/// COLLAB-01: every project a user belongs to (owner or otherwise).
pub fn list_projects_for_user(conn: &Connection, user_id: &str) -> Result<Vec<ProjectRecord>, ProjectError> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.created_by FROM project p
         JOIN project_member pm ON pm.project_id = p.id
         WHERE pm.user_id = ?1
         ORDER BY p.created_at",
    )?;
    let rows = stmt
        .query_map(params![user_id], |row| {
            Ok(ProjectRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                created_by: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn add_member(
    conn: &Connection,
    project_id: &str,
    user_id: &str,
    role: &str,
) -> Result<ProjectMemberRecord, ProjectError> {
    let id = mds_db::new_uuid();
    conn.execute(
        "INSERT INTO project_member (id, project_id, user_id, role) VALUES (?1, ?2, ?3, ?4)",
        params![id, project_id, user_id, role],
    )?;
    Ok(ProjectMemberRecord {
        id,
        project_id: project_id.to_string(),
        user_id: user_id.to_string(),
        role: role.to_string(),
    })
}

pub fn list_members(conn: &Connection, project_id: &str) -> Result<Vec<ProjectMemberRecord>, ProjectError> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, user_id, role FROM project_member
         WHERE project_id = ?1 ORDER BY created_at",
    )?;
    let rows = stmt
        .query_map(params![project_id], |row| {
            Ok(ProjectMemberRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                user_id: row.get(2)?,
                role: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn update_member_role(
    conn: &Connection,
    project_id: &str,
    user_id: &str,
    role: &str,
) -> Result<(), ProjectError> {
    let n = conn.execute(
        "UPDATE project_member SET role = ?1 WHERE project_id = ?2 AND user_id = ?3",
        params![role, project_id, user_id],
    )?;
    if n == 0 {
        return Err(ProjectError::NotAMember {
            project_id: project_id.to_string(),
            user_id: user_id.to_string(),
        });
    }
    Ok(())
}

pub fn remove_member(conn: &Connection, project_id: &str, user_id: &str) -> Result<(), ProjectError> {
    let n = conn.execute(
        "DELETE FROM project_member WHERE project_id = ?1 AND user_id = ?2",
        params![project_id, user_id],
    )?;
    if n == 0 {
        return Err(ProjectError::NotAMember {
            project_id: project_id.to_string(),
            user_id: user_id.to_string(),
        });
    }
    Ok(())
}

/// SEC-03 groundwork: a membership check. Enforcing it against any
/// particular action is an app-layer concern that doesn't exist yet.
pub fn is_member(conn: &Connection, project_id: &str, user_id: &str) -> Result<bool, ProjectError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM project_member WHERE project_id = ?1 AND user_id = ?2",
        params![project_id, user_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

// -- SEC-03 enforcement: resolving "what project (if any) owns this row" --
//
// `document.project_id` is nullable — a document isn't required to belong
// to a project — so every resolver here returns `Option<String>`, and
// `user_can_access_document`/`user_can_access_via_document` both treat
// "no project" as "no membership boundary to check," matching how every
// other feature already behaved before this module existed. An id that
// doesn't resolve at all (row not found, or a link like
// `takeoff_item.measurement_id` that's absent) is also let through rather
// than denied — the caller's own lookup still 404s on that id, which is a
// clearer error than an authorization one for a resource that isn't there.

pub fn project_id_for_document(conn: &Connection, document_id: &str) -> Result<Option<String>, ProjectError> {
    conn.query_row(
        "SELECT project_id FROM document WHERE id = ?1",
        params![document_id],
        |row| row.get(0),
    )
    .optional()
    .map(Option::flatten)
    .map_err(Into::into)
}

pub fn document_id_for_page(conn: &Connection, page_id: &str) -> Result<Option<String>, ProjectError> {
    conn.query_row(
        "SELECT document_id FROM page WHERE id = ?1",
        params![page_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn document_id_for_markup(conn: &Connection, markup_id: &str) -> Result<Option<String>, ProjectError> {
    conn.query_row(
        "SELECT p.document_id FROM markup m JOIN page p ON p.id = m.page_id WHERE m.id = ?1",
        params![markup_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn document_id_for_markup_comment(conn: &Connection, comment_id: &str) -> Result<Option<String>, ProjectError> {
    conn.query_row(
        "SELECT p.document_id FROM markup_comment c
         JOIN markup m ON m.id = c.markup_id
         JOIN page p ON p.id = m.page_id
         WHERE c.id = ?1",
        params![comment_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn document_id_for_measurement(conn: &Connection, measurement_id: &str) -> Result<Option<String>, ProjectError> {
    conn.query_row(
        "SELECT p.document_id FROM measurement me JOIN page p ON p.id = me.page_id WHERE me.id = ?1",
        params![measurement_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn document_id_for_rfi(conn: &Connection, rfi_id: &str) -> Result<Option<String>, ProjectError> {
    conn.query_row("SELECT document_id FROM rfi WHERE id = ?1", params![rfi_id], |row| row.get(0))
        .optional()
        .map_err(Into::into)
}

pub fn document_id_for_recovery_snapshot(conn: &Connection, snapshot_id: &str) -> Result<Option<String>, ProjectError> {
    conn.query_row(
        "SELECT document_id FROM recovery_state WHERE id = ?1",
        params![snapshot_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

/// TAKE-02's `measurement_id` is optional, so an item created without one
/// has no reachable document/project — `takeoff::list_for_document` (see
/// that crate) already excludes such items from any document-scoped view
/// for the same reason, so there's nothing new to check here either.
pub fn document_id_for_takeoff_item(conn: &Connection, item_id: &str) -> Result<Option<String>, ProjectError> {
    conn.query_row(
        "SELECT p.document_id FROM takeoff_item ti
         JOIN measurement me ON me.id = ti.measurement_id
         JOIN page p ON p.id = me.page_id
         WHERE ti.id = ?1",
        params![item_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

/// SEC-03: can `user_id` act on `document_id`? See the module note above
/// for the "no project / unresolved id" allow-through rule.
pub fn user_can_access_document(conn: &Connection, document_id: &str, user_id: &str) -> Result<bool, ProjectError> {
    match project_id_for_document(conn, document_id)? {
        Some(project_id) => is_member(conn, &project_id, user_id),
        None => Ok(true),
    }
}

/// Same as [`user_can_access_document`], but starting from an already-resolved
/// (and possibly absent) document id — the shape every `document_id_for_*`
/// resolver above returns, so app-layer callers can chain resolve+check in
/// one expression.
pub fn user_can_access_via_document(
    conn: &Connection,
    document_id: Option<String>,
    user_id: &str,
) -> Result<bool, ProjectError> {
    match document_id {
        Some(doc_id) => user_can_access_document(conn, &doc_id, user_id),
        None => Ok(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_db() -> Connection {
        mds_db::open_and_migrate(":memory:").unwrap()
    }

    #[test]
    fn get_user_by_email_finds_existing_and_errors_on_unknown() {
        let conn = open_test_db();
        let created = create_user(&conn, "owner@x.com", "Owner").unwrap();

        assert_eq!(get_user_by_email(&conn, "owner@x.com").unwrap(), created);
        assert!(matches!(
            get_user_by_email(&conn, "nobody@x.com"),
            Err(ProjectError::UserNotFound(_))
        ));
    }

    #[test]
    fn create_project_adds_creator_as_owner_member() {
        let mut conn = open_test_db();
        let owner = create_user(&conn, "owner@x.com", "Owner").unwrap();

        let project = create_project(&mut conn, "Rebar Job 1", &owner.id).unwrap();
        assert_eq!(get_project(&conn, &project.id).unwrap(), project);

        let members = list_members(&conn, &project.id).unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, owner.id);
        assert_eq!(members[0].role, "owner");
    }

    #[test]
    fn add_list_update_and_remove_members() {
        let mut conn = open_test_db();
        let owner = create_user(&conn, "owner@x.com", "Owner").unwrap();
        let editor = create_user(&conn, "editor@x.com", "Editor").unwrap();
        let project = create_project(&mut conn, "Rebar Job 1", &owner.id).unwrap();

        add_member(&conn, &project.id, &editor.id, "editor").unwrap();
        let members = list_members(&conn, &project.id).unwrap();
        assert_eq!(members.len(), 2);
        assert!(is_member(&conn, &project.id, &editor.id).unwrap());

        update_member_role(&conn, &project.id, &editor.id, "viewer").unwrap();
        let updated = list_members(&conn, &project.id).unwrap();
        assert_eq!(
            updated.iter().find(|m| m.user_id == editor.id).unwrap().role,
            "viewer"
        );

        remove_member(&conn, &project.id, &editor.id).unwrap();
        assert!(!is_member(&conn, &project.id, &editor.id).unwrap());
        assert_eq!(list_members(&conn, &project.id).unwrap().len(), 1);
    }

    #[test]
    fn adding_the_same_member_twice_is_rejected() {
        let mut conn = open_test_db();
        let owner = create_user(&conn, "owner@x.com", "Owner").unwrap();
        let member = create_user(&conn, "m@x.com", "M").unwrap();
        let project = create_project(&mut conn, "Rebar Job 1", &owner.id).unwrap();

        add_member(&conn, &project.id, &member.id, "editor").unwrap();
        let err = add_member(&conn, &project.id, &member.id, "editor").unwrap_err();
        assert!(matches!(err, ProjectError::Sqlite(_)));
    }

    #[test]
    fn removing_or_updating_a_non_member_errors() {
        let mut conn = open_test_db();
        let owner = create_user(&conn, "owner@x.com", "Owner").unwrap();
        let outsider = create_user(&conn, "out@x.com", "Out").unwrap();
        let project = create_project(&mut conn, "Rebar Job 1", &owner.id).unwrap();

        assert!(matches!(
            remove_member(&conn, &project.id, &outsider.id),
            Err(ProjectError::NotAMember { .. })
        ));
        assert!(matches!(
            update_member_role(&conn, &project.id, &outsider.id, "editor"),
            Err(ProjectError::NotAMember { .. })
        ));
    }

    #[test]
    fn list_projects_for_user_only_returns_their_projects() {
        let mut conn = open_test_db();
        let alice = create_user(&conn, "alice@x.com", "Alice").unwrap();
        let bob = create_user(&conn, "bob@x.com", "Bob").unwrap();

        let alices_project = create_project(&mut conn, "Alice's Job", &alice.id).unwrap();
        let bobs_project = create_project(&mut conn, "Bob's Job", &bob.id).unwrap();
        add_member(&conn, &bobs_project.id, &alice.id, "viewer").unwrap();

        let alices_projects = list_projects_for_user(&conn, &alice.id).unwrap();
        assert_eq!(alices_projects, vec![alices_project, bobs_project.clone()]);

        let bobs_projects = list_projects_for_user(&conn, &bob.id).unwrap();
        assert_eq!(bobs_projects, vec![bobs_project]);
    }

    /// Full fixture down through a markup/measurement/rfi/takeoff_item, all
    /// hanging off one project-owned document — exercises every
    /// `document_id_for_*` resolver's join in one pass.
    fn fixture_full_chain(conn: &mut Connection) -> (String, String, String, String, String, String, String) {
        let owner = create_user(conn, "owner@x.com", "Owner").unwrap();
        let project = create_project(conn, "Rebar Job 1", &owner.id).unwrap();
        let doc_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO document (id, project_id, file_path, title) VALUES (?1, ?2, '/tmp/x.pdf', 'X')",
            params![doc_id, project.id],
        )
        .unwrap();
        let page_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO page (id, document_id, page_number, width, height) VALUES (?1, ?2, 1, 100.0, 200.0)",
            params![page_id, doc_id],
        )
        .unwrap();
        let markup_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO markup (id, page_id, type, geometry_json, style_json) VALUES (?1, ?2, 'text', '{}', '{}')",
            params![markup_id, page_id],
        )
        .unwrap();
        let comment_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO markup_comment (id, markup_id, text) VALUES (?1, ?2, 'hi')",
            params![comment_id, markup_id],
        )
        .unwrap();
        let measurement_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO measurement (id, page_id, type, geometry_json, value, unit) VALUES (?1, ?2, 'count', '[]', 3.0, 'each')",
            params![measurement_id, page_id],
        )
        .unwrap();
        let rfi_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO rfi (id, document_id, number, title) VALUES (?1, ?2, 1, 'RFI 1')",
            params![rfi_id, doc_id],
        )
        .unwrap();
        let takeoff_item_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO takeoff_item (id, measurement_id, description, quantity, unit) VALUES (?1, ?2, 'rebar', 1.0, 'ea')",
            params![takeoff_item_id, measurement_id],
        )
        .unwrap();
        (project.id, doc_id, page_id, markup_id, comment_id, measurement_id, rfi_id)
    }

    #[test]
    fn resolvers_join_from_every_starting_point_to_the_owning_document() {
        let mut conn = open_test_db();
        let (_, doc_id, page_id, markup_id, comment_id, measurement_id, rfi_id) = fixture_full_chain(&mut conn);

        assert_eq!(document_id_for_page(&conn, &page_id).unwrap(), Some(doc_id.clone()));
        assert_eq!(document_id_for_markup(&conn, &markup_id).unwrap(), Some(doc_id.clone()));
        assert_eq!(document_id_for_markup_comment(&conn, &comment_id).unwrap(), Some(doc_id.clone()));
        assert_eq!(document_id_for_measurement(&conn, &measurement_id).unwrap(), Some(doc_id.clone()));
        assert_eq!(document_id_for_rfi(&conn, &rfi_id).unwrap(), Some(doc_id.clone()));

        // Unresolvable ids (row doesn't exist) come back `None`, not an error.
        assert_eq!(document_id_for_page(&conn, "missing").unwrap(), None);
    }

    #[test]
    fn takeoff_item_without_a_measurement_link_has_no_resolvable_document() {
        let mut conn = open_test_db();
        let owner = create_user(&conn, "owner@x.com", "Owner").unwrap();
        create_project(&mut conn, "Rebar Job 1", &owner.id).unwrap();
        let unlinked_item_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO takeoff_item (id, description, quantity, unit) VALUES (?1, 'misc', 1.0, 'ea')",
            params![unlinked_item_id],
        )
        .unwrap();
        assert_eq!(document_id_for_takeoff_item(&conn, &unlinked_item_id).unwrap(), None);
    }

    #[test]
    fn user_can_access_document_checks_membership_only_when_a_project_owns_it() {
        let mut conn = open_test_db();
        let (_, doc_id, ..) = fixture_full_chain(&mut conn);
        let outsider = create_user(&conn, "out@x.com", "Out").unwrap();
        let owner = get_user_by_email(&conn, "owner@x.com").unwrap();

        assert!(user_can_access_document(&conn, &doc_id, &owner.id).unwrap());
        assert!(!user_can_access_document(&conn, &doc_id, &outsider.id).unwrap());

        // A project-less document has no membership boundary at all.
        let unowned_doc_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO document (id, file_path, title) VALUES (?1, '/tmp/y.pdf', 'Y')",
            params![unowned_doc_id],
        )
        .unwrap();
        assert!(user_can_access_document(&conn, &unowned_doc_id, &outsider.id).unwrap());

        // An id that doesn't resolve at all is allowed through too — the
        // caller's own lookup is what should 404 on it.
        assert!(user_can_access_via_document(&conn, None, &outsider.id).unwrap());
    }
}
