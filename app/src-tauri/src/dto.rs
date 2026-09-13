//! DTOs at the Application/Presentation IPC boundary. The domain crates
//! (`project`, `document`, `measurement`, `takeoff`) deliberately don't
//! derive `Serialize` on their own record types — see each crate's module
//! docs — so wire-format shapes live here instead. `markup`'s
//! `MarkupType`/`MarkupGeometry`/`MarkupStyle` are the one exception: they
//! already derive `Serialize`/`Deserialize` for their own JSON persistence
//! (see `crates/markup`), so DTOs below embed them directly rather than
//! re-flattening equivalent fields. `rfi::RfiStatus` does the same, since
//! it also needs to flow as a Tauri command parameter.

#[derive(serde::Serialize)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub display_name: String,
}

impl From<project::UserRecord> for UserDto {
    fn from(u: project::UserRecord) -> Self {
        Self {
            id: u.id,
            email: u.email,
            display_name: u.display_name,
        }
    }
}

#[derive(serde::Serialize)]
pub struct ProjectDto {
    pub id: String,
    pub name: String,
    pub created_by: String,
}

impl From<project::ProjectRecord> for ProjectDto {
    fn from(p: project::ProjectRecord) -> Self {
        Self {
            id: p.id,
            name: p.name,
            created_by: p.created_by,
        }
    }
}

#[derive(serde::Serialize)]
pub struct ProjectMemberDto {
    pub id: String,
    pub project_id: String,
    pub user_id: String,
    pub role: String,
}

impl From<project::ProjectMemberRecord> for ProjectMemberDto {
    fn from(m: project::ProjectMemberRecord) -> Self {
        Self {
            id: m.id,
            project_id: m.project_id,
            user_id: m.user_id,
            role: m.role,
        }
    }
}

#[derive(serde::Serialize)]
pub struct DocumentDto {
    pub id: String,
    pub project_id: Option<String>,
    pub file_path: String,
    pub title: String,
}

impl From<document::DocumentRecord> for DocumentDto {
    fn from(d: document::DocumentRecord) -> Self {
        Self {
            id: d.id,
            project_id: d.project_id,
            file_path: d.file_path,
            title: d.title,
        }
    }
}

#[derive(serde::Serialize)]
pub struct PageDto {
    pub id: String,
    pub document_id: String,
    pub page_number: i64,
    pub width: f64,
    pub height: f64,
    pub rotation: i64,
}

impl From<document::PageRecord> for PageDto {
    fn from(p: document::PageRecord) -> Self {
        Self {
            id: p.id,
            document_id: p.document_id,
            page_number: p.page_number,
            width: p.width,
            height: p.height,
            rotation: p.rotation,
        }
    }
}

#[derive(serde::Serialize)]
pub struct DocumentVersionDto {
    pub id: String,
    pub document_id: String,
    pub version_number: i64,
    pub file_snapshot_path: String,
    pub created_by: Option<String>,
}

impl From<document::DocumentVersionRecord> for DocumentVersionDto {
    fn from(v: document::DocumentVersionRecord) -> Self {
        Self {
            id: v.id,
            document_id: v.document_id,
            version_number: v.version_number,
            file_snapshot_path: v.file_snapshot_path,
            created_by: v.created_by,
        }
    }
}

#[derive(serde::Serialize)]
pub struct MarkupDto {
    pub id: String,
    pub page_id: String,
    pub markup_type: markup::MarkupType,
    pub geometry: markup::MarkupGeometry,
    pub style: markup::MarkupStyle,
    pub author: Option<String>,
    pub locked: bool,
    pub hidden: bool,
}

#[derive(serde::Serialize)]
pub struct UndoStatusDto {
    pub can_undo: bool,
    pub can_redo: bool,
}

impl From<markup::Markup> for MarkupDto {
    fn from(m: markup::Markup) -> Self {
        Self {
            id: m.id,
            page_id: m.page_id,
            markup_type: m.markup_type,
            geometry: m.geometry,
            style: m.style,
            author: m.author,
            locked: m.locked,
            hidden: m.hidden,
        }
    }
}

#[derive(serde::Serialize)]
pub struct MarkupCommentDto {
    pub id: String,
    pub markup_id: String,
    pub author: Option<String>,
    pub text: String,
}

impl From<markup::MarkupComment> for MarkupCommentDto {
    fn from(c: markup::MarkupComment) -> Self {
        Self {
            id: c.id,
            markup_id: c.markup_id,
            author: c.author,
            text: c.text,
        }
    }
}

#[derive(serde::Serialize)]
pub struct ScaleDto {
    pub id: String,
    pub page_id: String,
    pub inches_per_page_unit: f64,
    pub unit_system: String,
    pub calibrated_by: Option<String>,
}

impl From<measurement::CalibratedScale> for ScaleDto {
    fn from(s: measurement::CalibratedScale) -> Self {
        Self {
            id: s.id,
            page_id: s.page_id,
            inches_per_page_unit: s.scale.inches_per_page_unit(),
            unit_system: s.unit_system,
            calibrated_by: s.calibrated_by,
        }
    }
}

#[derive(serde::Serialize)]
pub struct MeasurementDto {
    pub id: String,
    pub page_id: String,
    pub scale_id: Option<String>,
    pub measurement_type: measurement::MeasurementType,
    pub geometry: Vec<(f64, f64)>,
    pub value: f64,
    pub unit: String,
    pub label: Option<String>,
}

impl From<measurement::Measurement> for MeasurementDto {
    fn from(m: measurement::Measurement) -> Self {
        Self {
            id: m.id,
            page_id: m.page_id,
            scale_id: m.scale_id,
            measurement_type: m.measurement_type,
            geometry: m.geometry,
            value: m.value,
            unit: m.unit,
            label: m.label,
        }
    }
}

#[derive(serde::Serialize)]
pub struct TakeoffItemDto {
    pub id: String,
    pub measurement_id: Option<String>,
    pub description: String,
    pub quantity: f64,
    pub unit: String,
    pub cost_per_unit: Option<f64>,
    pub notes: Option<String>,
    pub total_cost: Option<f64>,
}

#[derive(serde::Serialize)]
pub struct RfiDto {
    pub id: String,
    pub document_id: String,
    pub page_id: Option<String>,
    pub markup_id: Option<String>,
    pub number: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: rfi::RfiStatus,
    pub response: Option<String>,
    pub created_by: Option<String>,
}

impl From<rfi::Rfi> for RfiDto {
    fn from(r: rfi::Rfi) -> Self {
        Self {
            id: r.id,
            document_id: r.document_id,
            page_id: r.page_id,
            markup_id: r.markup_id,
            number: r.number,
            title: r.title,
            description: r.description,
            status: r.status,
            response: r.response,
            created_by: r.created_by,
        }
    }
}

#[derive(serde::Serialize)]
pub struct RecoverySnapshotDto {
    pub id: String,
    pub document_id: String,
    pub snapshot_path: String,
    pub created_at: String,
}

impl From<recovery::RecoverySnapshot> for RecoverySnapshotDto {
    fn from(s: recovery::RecoverySnapshot) -> Self {
        Self {
            id: s.id,
            document_id: s.document_id,
            snapshot_path: s.snapshot_path,
            created_at: s.created_at,
        }
    }
}

impl From<takeoff::TakeoffItem> for TakeoffItemDto {
    fn from(item: takeoff::TakeoffItem) -> Self {
        let total_cost = item.total_cost();
        Self {
            id: item.id,
            measurement_id: item.measurement_id,
            description: item.description,
            quantity: item.quantity,
            unit: item.unit,
            cost_per_unit: item.cost_per_unit,
            notes: item.notes,
            total_cost,
        }
    }
}
