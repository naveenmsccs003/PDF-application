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
}
