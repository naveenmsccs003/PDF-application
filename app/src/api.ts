// Typed wrappers around the Tauri IPC commands defined in
// app/src-tauri/src/commands/*.rs. Two different naming conventions are in
// play here, and it's easy to mix them up:
//   - Top-level command arguments are camelCased by Tauri's #[tauri::command]
//     macro (Rust `page_id` -> JS `{ pageId }`).
//   - Fields *inside* a returned/passed struct (DTOs in dto.rs, and
//     markup's MarkupType/MarkupGeometry/MarkupStyle) keep their Rust
//     snake_case names, because those types don't have
//     #[serde(rename_all = "camelCase")] applied.
import { invoke } from "@tauri-apps/api/core";

export type Point = [number, number];

export interface UserDto {
  id: string;
  email: string;
  display_name: string;
}

export interface ProjectDto {
  id: string;
  name: string;
  created_by: string;
}

export interface ProjectMemberDto {
  id: string;
  project_id: string;
  user_id: string;
  role: string;
}

export interface DocumentDto {
  id: string;
  project_id: string | null;
  file_path: string;
  title: string;
}

export interface PageDto {
  id: string;
  document_id: string;
  page_number: number;
  width: number;
  height: number;
  rotation: number;
}

export interface DocumentVersionDto {
  id: string;
  document_id: string;
  version_number: number;
  file_snapshot_path: string;
  created_by: string | null;
}

export type MarkupType = "Text" | "Rectangle" | "Cloud" | "Line" | "Arrow";

export interface MarkupGeometry {
  points: Point[];
}

export interface MarkupStyle {
  color: string | null;
  stroke_width: number | null;
  text: string | null;
}

export interface MarkupDto {
  id: string;
  page_id: string;
  markup_type: MarkupType;
  geometry: MarkupGeometry;
  style: MarkupStyle;
  author: string | null;
  locked: boolean;
  hidden: boolean;
}

export interface MarkupCommentDto {
  id: string;
  markup_id: string;
  author: string | null;
  text: string;
}

export interface ScaleDto {
  id: string;
  page_id: string;
  inches_per_page_unit: number;
  unit_system: string;
  calibrated_by: string | null;
}

export type MeasurementType = "Length" | "Area" | "Count";
export type LengthUnitCode = "in" | "ft" | "mm" | "cm" | "m";
export type AreaUnitCode = "sq_in" | "sq_ft" | "sq_m";

export interface MeasurementDto {
  id: string;
  page_id: string;
  scale_id: string | null;
  measurement_type: MeasurementType;
  geometry: Point[];
  value: number;
  unit: string;
  label: string | null;
}

export interface TakeoffItemDto {
  id: string;
  measurement_id: string | null;
  description: string;
  quantity: number;
  unit: string;
  cost_per_unit: number | null;
  notes: string | null;
  total_cost: number | null;
}

export type RfiStatus = "Open" | "Answered" | "Closed";

export interface RfiDto {
  id: string;
  document_id: string;
  page_id: string | null;
  markup_id: string | null;
  number: number;
  title: string;
  description: string | null;
  status: RfiStatus;
  response: string | null;
  created_by: string | null;
}

// -- project --

export const createUser = (email: string, displayName: string) =>
  invoke<UserDto>("create_user", { email, displayName });

export const getUserByEmail = (email: string) => invoke<UserDto>("get_user_by_email", { email });

export const createProject = (name: string, createdBy: string) =>
  invoke<ProjectDto>("create_project", { name, createdBy });

export const listProjectsForUser = (userId: string) =>
  invoke<ProjectDto[]>("list_projects_for_user", { userId });

export const addProjectMember = (projectId: string, userId: string, role: string) =>
  invoke<ProjectMemberDto>("add_project_member", { projectId, userId, role });

export const listProjectMembers = (projectId: string) =>
  invoke<ProjectMemberDto[]>("list_project_members", { projectId });

export const removeProjectMember = (projectId: string, userId: string) =>
  invoke<void>("remove_project_member", { projectId, userId });

// -- document / pdf --

export const importPdfDocument = (path: string, title: string, projectId: string | null) =>
  invoke<DocumentDto>("import_pdf_document", { path, title, projectId });

export const renderPageThumbnail = (documentId: string, pageNumber: number, width: number) =>
  invoke<string>("render_page_thumbnail", { documentId, pageNumber, width });

export const exportFlattenedPdf = (documentId: string, outputPath: string) =>
  invoke<void>("export_flattened_pdf", { documentId, outputPath });

export const exportHandoffPackage = (documentId: string, outputDir: string) =>
  invoke<void>("export_handoff_package", { documentId, outputDir });

// -- recovery (REL-01/02) --

export interface RecoverySnapshotDto {
  id: string;
  document_id: string;
  snapshot_path: string;
  created_at: string;
}

export const autosaveSnapshot = (documentId: string) =>
  invoke<RecoverySnapshotDto>("autosave_snapshot", { documentId });

export const listRecoverySnapshots = (documentId: string) =>
  invoke<RecoverySnapshotDto[]>("list_recovery_snapshots", { documentId });

export const restoreRecoverySnapshot = (id: string, outputPath: string) =>
  invoke<void>("restore_recovery_snapshot", { id, outputPath });

export const discardRecoverySnapshot = (id: string) => invoke<void>("discard_recovery_snapshot", { id });

export const listDocumentsForProject = (projectId: string) =>
  invoke<DocumentDto[]>("list_documents_for_project", { projectId });

export const listPages = (documentId: string) => invoke<PageDto[]>("list_pages", { documentId });

export const setPageRotation = (pageId: string, rotation: number) =>
  invoke<void>("set_page_rotation", { pageId, rotation });

export const reorderPages = (documentId: string, newOrder: string[]) =>
  invoke<void>("reorder_pages", { documentId, newOrder });

export const createDocumentVersion = (documentId: string, fileSnapshotPath: string, createdBy: string | null) =>
  invoke<DocumentVersionDto>("create_document_version", { documentId, fileSnapshotPath, createdBy });

export const listDocumentVersions = (documentId: string) =>
  invoke<DocumentVersionDto[]>("list_document_versions", { documentId });

// -- markup --

export const createMarkup = (
  pageId: string,
  markupType: MarkupType,
  geometry: MarkupGeometry,
  style: MarkupStyle,
  author: string | null,
) => invoke<MarkupDto>("create_markup", { pageId, markupType, geometry, style, author });

export const listMarkupsByPage = (pageId: string) => invoke<MarkupDto[]>("list_markups_by_page", { pageId });

export const updateMarkupGeometry = (id: string, markupType: MarkupType, geometry: MarkupGeometry) =>
  invoke<void>("update_markup_geometry", { id, markupType, geometry });

export const updateMarkupStyle = (id: string, style: MarkupStyle) =>
  invoke<void>("update_markup_style", { id, style });

export const setMarkupLocked = (id: string, locked: boolean) => invoke<void>("set_markup_locked", { id, locked });

export const setMarkupHidden = (id: string, hidden: boolean) => invoke<void>("set_markup_hidden", { id, hidden });

export const deleteMarkup = (id: string) => invoke<void>("delete_markup", { id });

export const addMarkupComment = (markupId: string, author: string | null, text: string) =>
  invoke<MarkupCommentDto>("add_markup_comment", { markupId, author, text });

export const listMarkupComments = (markupId: string) =>
  invoke<MarkupCommentDto[]>("list_markup_comments", { markupId });

export const deleteMarkupComment = (id: string) => invoke<void>("delete_markup_comment", { id });

// -- markup undo/redo (MARK-06) --

export interface UndoStatusDto {
  can_undo: boolean;
  can_redo: boolean;
}

export const undoMarkup = (pageId: string) => invoke<boolean>("undo_markup", { pageId });

export const redoMarkup = (pageId: string) => invoke<boolean>("redo_markup", { pageId });

export const markupUndoStatus = (pageId: string) => invoke<UndoStatusDto>("markup_undo_status", { pageId });

// -- measurement --

export const calibrateScale = (
  pageId: string,
  p1: Point,
  p2: Point,
  knownRealWorldInches: number,
  unitSystem: string,
  calibratedBy: string | null,
) => invoke<ScaleDto>("calibrate_scale", { pageId, p1, p2, knownRealWorldInches, unitSystem, calibratedBy });

export const latestScaleForPage = (pageId: string) => invoke<ScaleDto | null>("latest_scale_for_page", { pageId });

export const recordLength = (
  pageId: string,
  scaleId: string,
  p1: Point,
  p2: Point,
  unit: LengthUnitCode,
  label: string | null,
) => invoke<MeasurementDto>("record_length", { pageId, scaleId, p1, p2, unit, label });

export const recordArea = (
  pageId: string,
  scaleId: string,
  points: Point[],
  unit: AreaUnitCode,
  label: string | null,
) => invoke<MeasurementDto>("record_area", { pageId, scaleId, points, unit, label });

export const recordCount = (pageId: string, markers: Point[], label: string | null) =>
  invoke<MeasurementDto>("record_count", { pageId, markers, label });

export const listMeasurementsByPage = (pageId: string) =>
  invoke<MeasurementDto[]>("list_measurements_by_page", { pageId });

export const deleteMeasurement = (id: string) => invoke<void>("delete_measurement", { id });

// -- takeoff --

export const createTakeoffItem = (
  description: string,
  quantity: number,
  unit: string,
  measurementId: string | null,
  costPerUnit: number | null,
  notes: string | null,
) => invoke<TakeoffItemDto>("create_takeoff_item", { description, quantity, unit, measurementId, costPerUnit, notes });

export const listTakeoffForDocument = (documentId: string) =>
  invoke<TakeoffItemDto[]>("list_takeoff_for_document", { documentId });

export const updateTakeoffItem = (
  id: string,
  quantity: number,
  unit: string,
  costPerUnit: number | null,
  notes: string | null,
) => invoke<void>("update_takeoff_item", { id, quantity, unit, costPerUnit, notes });

export const deleteTakeoffItem = (id: string) => invoke<void>("delete_takeoff_item", { id });

export const exportTakeoffCsv = (documentId: string) => invoke<string>("export_takeoff_csv", { documentId });

// -- rfi --

export const createRfi = (
  documentId: string,
  pageId: string | null,
  markupId: string | null,
  title: string,
  description: string | null,
  createdBy: string | null,
) => invoke<RfiDto>("create_rfi", { documentId, pageId, markupId, title, description, createdBy });

export const listRfisForDocument = (documentId: string) => invoke<RfiDto[]>("list_rfis_for_document", { documentId });

export const setRfiStatus = (id: string, status: RfiStatus, response: string | null) =>
  invoke<void>("set_rfi_status", { id, status, response });
