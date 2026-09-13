import { useEffect, useRef, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import type {
  DocumentDto,
  MarkupCommentDto,
  MarkupDto,
  MarkupType,
  MeasurementDto,
  PageDto,
  Point,
  ProjectDto,
  ProjectMemberDto,
  ScaleDto,
  TakeoffItemDto,
  UserDto,
} from "./api";
import "./App.css";

/** Parses "x,y" into a Point. Throws (caller shows the error) on bad input. */
function parsePoint(input: string): Point {
  const parts = input.split(",").map((s) => Number(s.trim()));
  if (parts.length !== 2 || parts.some((n) => Number.isNaN(n))) {
    throw new Error(`expected "x,y", got "${input}"`);
  }
  return [parts[0], parts[1]];
}

/** Parses "x1,y1 x2,y2 ..." into Point[]. */
function parsePoints(input: string): Point[] {
  return input
    .trim()
    .split(/\s+/)
    .filter((s) => s.length > 0)
    .map(parsePoint);
}

function ErrorBanner({ error, onDismiss }: { error: string | null; onDismiss: () => void }) {
  if (!error) return null;
  return (
    <div className="error-banner">
      <span>{error}</span>
      <button onClick={onDismiss}>dismiss</button>
    </div>
  );
}

export default function App() {
  const [error, setError] = useState<string | null>(null);
  const runAction = async (fn: () => Promise<void>) => {
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    }
  };

  // -- identity --
  const [user, setUser] = useState<UserDto | null>(null);
  const [emailInput, setEmailInput] = useState("");
  const [nameInput, setNameInput] = useState("");

  const signIn = () =>
    runAction(async () => {
      try {
        setUser(await api.getUserByEmail(emailInput));
      } catch {
        setUser(await api.createUser(emailInput, nameInput || emailInput));
      }
    });

  // -- projects --
  const [projects, setProjects] = useState<ProjectDto[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);

  useEffect(() => {
    if (!user) return;
    runAction(async () => setProjects(await api.listProjectsForUser(user.id)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [user]);

  const selectedProject = projects.find((p) => p.id === selectedProjectId) ?? null;

  return (
    <main className="app">
      <h1>MDS Rebar — Backend Control Panel</h1>
      <p className="subtitle">
        Working UI over the real IPC layer (not mockups) — every action here calls into
        the Rust domain crates via Tauri commands. The page view renders the actual PDF
        and supports click-to-draw markup (rectangle/line/arrow/cloud/text) plus
        select/move/resize on existing shapes; everything else (projects, measurement,
        takeoff) is still a functional control panel rather than a polished editor.
        Zoom/pan isn't built yet.
      </p>
      <ErrorBanner error={error} onDismiss={() => setError(null)} />

      {!user ? (
        <section className="card">
          <h2>Sign in</h2>
          <p>Existing email signs you in; a new one creates an account.</p>
          <input placeholder="email" value={emailInput} onChange={(e) => setEmailInput(e.target.value)} />
          <input placeholder="display name (for new accounts)" value={nameInput} onChange={(e) => setNameInput(e.target.value)} />
          <button onClick={signIn} disabled={!emailInput}>
            Sign in
          </button>
        </section>
      ) : (
        <>
          <section className="card">
            <h2>
              Signed in as {user.display_name} <span className="muted">({user.email})</span>
            </h2>
            <button onClick={() => setUser(null)}>Sign out</button>
          </section>

          <ProjectsPanel
            user={user}
            projects={projects}
            selectedProjectId={selectedProjectId}
            onSelectProject={setSelectedProjectId}
            onProjectsChanged={(ps) => setProjects(ps)}
            runAction={runAction}
          />

          {selectedProject && <DocumentsPanel project={selectedProject} user={user} runAction={runAction} />}
        </>
      )}
    </main>
  );
}

// ---------------------------------------------------------------------------
// Projects (COLLAB-01 / DOC-06)
// ---------------------------------------------------------------------------

function ProjectsPanel({
  user,
  projects,
  selectedProjectId,
  onSelectProject,
  onProjectsChanged,
  runAction,
}: {
  user: UserDto;
  projects: ProjectDto[];
  selectedProjectId: string | null;
  onSelectProject: (id: string | null) => void;
  onProjectsChanged: (projects: ProjectDto[]) => void;
  runAction: (fn: () => Promise<void>) => Promise<void>;
}) {
  const [newProjectName, setNewProjectName] = useState("");
  const [members, setMembers] = useState<ProjectMemberDto[]>([]);
  const [memberEmail, setMemberEmail] = useState("");
  const [memberRole, setMemberRole] = useState("editor");

  const reloadMembers = (projectId: string) =>
    runAction(async () => setMembers(await api.listProjectMembers(projectId)));

  useEffect(() => {
    if (selectedProjectId) reloadMembers(selectedProjectId);
    else setMembers([]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedProjectId]);

  const createProject = () =>
    runAction(async () => {
      const project = await api.createProject(newProjectName, user.id);
      onProjectsChanged([...projects, project]);
      onSelectProject(project.id);
      setNewProjectName("");
    });

  const addMember = () =>
    runAction(async () => {
      if (!selectedProjectId) return;
      const target = await api.getUserByEmail(memberEmail);
      await api.addProjectMember(selectedProjectId, target.id, memberRole);
      await reloadMembers(selectedProjectId);
      setMemberEmail("");
    });

  const removeMember = (userId: string) =>
    runAction(async () => {
      if (!selectedProjectId) return;
      await api.removeProjectMember(selectedProjectId, userId);
      await reloadMembers(selectedProjectId);
    });

  return (
    <section className="card">
      <h2>Projects (COLLAB-01)</h2>
      <div className="row">
        <select value={selectedProjectId ?? ""} onChange={(e) => onSelectProject(e.target.value || null)}>
          <option value="">— select a project —</option>
          {projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        <input placeholder="new project name" value={newProjectName} onChange={(e) => setNewProjectName(e.target.value)} />
        <button onClick={createProject} disabled={!newProjectName}>
          Create project
        </button>
      </div>

      {selectedProjectId && (
        <div className="nested">
          <h3>Members</h3>
          <ul>
            {members.map((m) => (
              <li key={m.id}>
                {m.user_id === user.id ? "you" : m.user_id.slice(0, 8)} — {m.role}{" "}
                {m.user_id !== user.id && <button onClick={() => removeMember(m.user_id)}>remove</button>}
              </li>
            ))}
          </ul>
          <div className="row">
            <input placeholder="member email (must already have an account)" value={memberEmail} onChange={(e) => setMemberEmail(e.target.value)} />
            <select value={memberRole} onChange={(e) => setMemberRole(e.target.value)}>
              <option value="editor">editor</option>
              <option value="viewer">viewer</option>
            </select>
            <button onClick={addMember} disabled={!memberEmail}>
              Add member
            </button>
          </div>
        </div>
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------
// Documents (DOC-01/02/04/05)
// ---------------------------------------------------------------------------

function DocumentsPanel({
  project,
  user,
  runAction,
}: {
  project: ProjectDto;
  user: UserDto;
  runAction: (fn: () => Promise<void>) => Promise<void>;
}) {
  const [documents, setDocuments] = useState<DocumentDto[]>([]);
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(null);
  const [pages, setPages] = useState<PageDto[]>([]);
  const [thumbnails, setThumbnails] = useState<Record<string, string>>({});
  const [selectedPageId, setSelectedPageId] = useState<string | null>(null);
  const [importTitle, setImportTitle] = useState("");

  const reloadDocuments = () =>
    runAction(async () => setDocuments(await api.listDocumentsForProject(project.id)));

  useEffect(() => {
    reloadDocuments();
    setSelectedDocumentId(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [project.id]);

  const reloadPages = (documentId: string) =>
    runAction(async () => {
      const loaded = await api.listPages(documentId);
      setPages(loaded);
      for (const page of loaded) {
        api
          .renderPageThumbnail(documentId, page.page_number, 150)
          .then((uri) => setThumbnails((prev) => ({ ...prev, [page.id]: uri })))
          .catch(() => {
            /* PDF engine unavailable this session — thumbnail just stays blank */
          });
      }
    });

  useEffect(() => {
    if (selectedDocumentId) reloadPages(selectedDocumentId);
    else {
      setPages([]);
      setThumbnails({});
    }
    setSelectedPageId(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedDocumentId]);

  const importPdf = () =>
    runAction(async () => {
      const path = await openFileDialog({ multiple: false, filters: [{ name: "PDF", extensions: ["pdf"] }] });
      if (!path || typeof path !== "string") return;
      const filename = path.split("/").pop() ?? path;
      const title = importTitle || filename;
      const doc = await api.importPdfDocument(path, title, project.id);
      await reloadDocuments();
      setSelectedDocumentId(doc.id);
      setImportTitle("");
    });

  const rotatePage = (page: PageDto) =>
    runAction(async () => {
      const next = ((page.rotation + 90) % 360) as 0 | 90 | 180 | 270;
      await api.setPageRotation(page.id, next);
      if (selectedDocumentId) await reloadPages(selectedDocumentId);
    });

  const selectedDocument = documents.find((d) => d.id === selectedDocumentId) ?? null;
  const selectedPage = pages.find((p) => p.id === selectedPageId) ?? null;

  return (
    <section className="card">
      <h2>Documents (DOC-01/04/05)</h2>
      <div className="row">
        <select value={selectedDocumentId ?? ""} onChange={(e) => setSelectedDocumentId(e.target.value || null)}>
          <option value="">— select a document —</option>
          {documents.map((d) => (
            <option key={d.id} value={d.id}>
              {d.title}
            </option>
          ))}
        </select>
        <input placeholder="title for imported PDF (optional)" value={importTitle} onChange={(e) => setImportTitle(e.target.value)} />
        <button onClick={importPdf}>Import PDF…</button>
      </div>

      {selectedDocument && (
        <div className="nested">
          <h3>Pages</h3>
          <div className="page-grid">
            {pages.map((page) => (
              <div key={page.id} className={`page-thumb ${page.id === selectedPageId ? "selected" : ""}`} onClick={() => setSelectedPageId(page.id)}>
                {thumbnails[page.id] ? (
                  <img src={thumbnails[page.id]} alt={`page ${page.page_number}`} />
                ) : (
                  <div className="thumb-placeholder">no preview</div>
                )}
                <div className="page-label">
                  p.{page.page_number} ({page.rotation}°)
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    rotatePage(page);
                  }}
                >
                  rotate
                </button>
              </div>
            ))}
          </div>

          {selectedPage && <PagePanel page={selectedPage} user={user} runAction={runAction} />}

          <TakeoffPanel document={selectedDocument} runAction={runAction} />
        </div>
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------
// PDF page canvas — click-to-draw markup over the real rendered page
// (VIEW-01/02, MARK-01–04). Page-space coordinates are pixels-at-RENDER_WIDTH
// scaled by page.width/RENDER_WIDTH, so they stay in the same "page unit"
// space the manual measurement inputs below already use (page.width/height,
// as reported by PageDto, are PDF points) — consistent within this app even
// though the y-axis here is image-top-down rather than PDF's native
// bottom-up, since nothing yet round-trips these coordinates through a real
// PDF export.
// ---------------------------------------------------------------------------

const RENDER_WIDTH = 900;

type DrawTool = "select" | MarkupType;

function minPointsFor(type: MarkupType): number {
  if (type === "Text") return 1;
  if (type === "Cloud") return 3;
  return 2;
}

/** Smallest axis-aligned box containing every point, in page space. */
function boundingBox(points: Point[]): { minX: number; minY: number; maxX: number; maxY: number } {
  const xs = points.map((p) => p[0]);
  const ys = points.map((p) => p[1]);
  return { minX: Math.min(...xs), minY: Math.min(...ys), maxX: Math.max(...xs), maxY: Math.max(...ys) };
}

/** Point-in-shape hit test in page space, generous enough for thin lines/text. */
function hitTest(m: MarkupDto, p: Point, tolerancePageUnits: number): boolean {
  const pts = m.geometry.points;
  if (m.markup_type === "Text") {
    const [tx, ty] = pts[0];
    return Math.abs(p[0] - tx) < tolerancePageUnits * 6 && Math.abs(p[1] - ty) < tolerancePageUnits * 3;
  }
  if (m.markup_type === "Line" || m.markup_type === "Arrow") {
    const [[x1, y1], [x2, y2]] = pts;
    const len2 = (x2 - x1) ** 2 + (y2 - y1) ** 2;
    const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p[0] - x1) * (x2 - x1) + (p[1] - y1) * (y2 - y1)) / len2));
    const cx = x1 + t * (x2 - x1);
    const cy = y1 + t * (y2 - y1);
    return Math.hypot(p[0] - cx, p[1] - cy) < tolerancePageUnits;
  }
  // Rectangle/Cloud: bounding-box test is good enough at this pass's fidelity.
  const box = boundingBox(pts);
  return p[0] >= box.minX - tolerancePageUnits && p[0] <= box.maxX + tolerancePageUnits && p[1] >= box.minY - tolerancePageUnits && p[1] <= box.maxY + tolerancePageUnits;
}

/**
 * Points a shape exposes as draggable resize handles, in page space — index
 * matches `geometry.points` so a drag can write straight back to one entry.
 * Rectangle is stored as two diagonal corners (same pair the drag-to-draw
 * gesture produces), so dragging either one resizes it the same way drawing
 * did. Cloud/Text return none: multi-point cloud editing and text have no
 * single-corner resize that makes sense at this pass's scope — move only.
 */
function resizeHandles(m: MarkupDto): Point[] {
  if (m.markup_type === "Line" || m.markup_type === "Arrow" || m.markup_type === "Rectangle") {
    return m.geometry.points;
  }
  return [];
}

function PdfCanvas({
  page,
  markups,
  user,
  onCreated,
  runAction,
}: {
  page: PageDto;
  markups: MarkupDto[];
  user: UserDto;
  onCreated: () => void;
  runAction: (fn: () => Promise<void>) => Promise<void>;
}) {
  const [imageUri, setImageUri] = useState<string | null>(null);
  const [tool, setTool] = useState<DrawTool>("select");
  const [color, setColor] = useState("#e53935");
  const [dragStart, setDragStart] = useState<Point | null>(null);
  const [dragCurrent, setDragCurrent] = useState<Point | null>(null);
  const [cloudPoints, setCloudPoints] = useState<Point[]>([]);
  const [pendingTextPoint, setPendingTextPoint] = useState<Point | null>(null);
  const [pendingTextValue, setPendingTextValue] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [moveState, setMoveState] = useState<{ id: string; originPoints: Point[]; startPointer: Point } | null>(null);
  const [resizeState, setResizeState] = useState<{ id: string; pointIndex: number } | null>(null);
  const [livePoints, setLivePoints] = useState<Point[] | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  const renderedHeight = page.width > 0 ? (RENDER_WIDTH * page.height) / page.width : RENDER_WIDTH;
  const scale = page.width > 0 ? page.width / RENDER_WIDTH : 1;

  useEffect(() => {
    let cancelled = false;
    setImageUri(null);
    api
      .renderPageThumbnail(page.document_id, page.page_number, RENDER_WIDTH)
      .then((uri) => {
        if (!cancelled) setImageUri(uri);
      })
      .catch(() => {
        /* PDF engine unavailable this session — canvas stays blank, list view still works */
      });
    return () => {
      cancelled = true;
    };
  }, [page.id, page.document_id, page.page_number]);

  const toPagePoint = (clientX: number, clientY: number): Point => {
    const rect = svgRef.current!.getBoundingClientRect();
    return [(clientX - rect.left) * scale, (clientY - rect.top) * scale];
  };
  const toPixel = ([x, y]: Point): Point => [x / scale, y / scale];

  const commitShape = (type: MarkupType, points: Point[], text: string | null = null) =>
    runAction(async () => {
      if (points.length < minPointsFor(type)) return;
      const style = { color, stroke_width: type === "Text" ? null : 2, text };
      await api.createMarkup(page.id, type, { points }, style, user.id);
      onCreated();
    });

  const commitGeometry = (id: string, markupType: MarkupType, points: Point[]) =>
    runAction(async () => {
      await api.updateMarkupGeometry(id, markupType, { points });
      onCreated();
    });

  const selectTool = (t: DrawTool) => {
    setTool(t);
    setCloudPoints([]);
    setPendingTextPoint(null);
    setSelectedId(null);
    setMoveState(null);
    setResizeState(null);
    setLivePoints(null);
  };

  const handleMouseDown = (e: React.MouseEvent) => {
    const p = toPagePoint(e.clientX, e.clientY);
    if (tool === "select") {
      const tolerance = 8 * scale;
      const selected = markups.find((m) => m.id === selectedId);
      if (selected && !selected.locked) {
        const handleIdx = resizeHandles(selected).findIndex(
          (h) => Math.hypot(h[0] - p[0], h[1] - p[1]) < tolerance * 1.5,
        );
        if (handleIdx >= 0) {
          setResizeState({ id: selected.id, pointIndex: handleIdx });
          setLivePoints(selected.geometry.points);
          return;
        }
      }
      const hit = [...markups].reverse().find((m) => !m.hidden && hitTest(m, p, tolerance));
      setSelectedId(hit ? hit.id : null);
      if (hit && !hit.locked) {
        setMoveState({ id: hit.id, originPoints: hit.geometry.points, startPointer: p });
        setLivePoints(hit.geometry.points);
      }
      return;
    }
    if (tool === "Text") {
      setPendingTextPoint(p);
      setPendingTextValue("");
      return;
    }
    if (tool === "Cloud") {
      setCloudPoints((prev) => [...prev, p]);
      return;
    }
    setDragStart(p);
    setDragCurrent(p);
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (moveState) {
      const p = toPagePoint(e.clientX, e.clientY);
      const dx = p[0] - moveState.startPointer[0];
      const dy = p[1] - moveState.startPointer[1];
      setLivePoints(moveState.originPoints.map(([x, y]) => [x + dx, y + dy] as Point));
      return;
    }
    if (resizeState) {
      const p = toPagePoint(e.clientX, e.clientY);
      setLivePoints((prev) => {
        if (!prev) return prev;
        const next = [...prev];
        next[resizeState.pointIndex] = p;
        return next;
      });
      return;
    }
    if (!dragStart) return;
    setDragCurrent(toPagePoint(e.clientX, e.clientY));
  };

  const handleMouseUp = () => {
    if (moveState) {
      const markupType = markups.find((m) => m.id === moveState.id)?.markup_type;
      const changed = livePoints && JSON.stringify(livePoints) !== JSON.stringify(moveState.originPoints);
      if (markupType && changed && livePoints) commitGeometry(moveState.id, markupType, livePoints);
      setMoveState(null);
      setLivePoints(null);
      return;
    }
    if (resizeState) {
      const original = markups.find((m) => m.id === resizeState.id);
      const changed = original && livePoints && JSON.stringify(livePoints) !== JSON.stringify(original.geometry.points);
      if (original && changed && livePoints) commitGeometry(resizeState.id, original.markup_type, livePoints);
      setResizeState(null);
      setLivePoints(null);
      return;
    }
    if (!dragStart || !dragCurrent) return;
    commitShape(tool as MarkupType, [dragStart, dragCurrent]);
    setDragStart(null);
    setDragCurrent(null);
  };

  const finishCloud = () => {
    if (cloudPoints.length >= 3) commitShape("Cloud", cloudPoints);
    setCloudPoints([]);
  };

  const commitPendingText = () => {
    if (pendingTextPoint && pendingTextValue.trim()) {
      commitShape("Text", [pendingTextPoint], pendingTextValue.trim());
    }
    setPendingTextPoint(null);
  };

  const isDragTool = tool !== "select" && tool !== "Cloud" && tool !== "Text";

  return (
    <div>
      <div className="row">
        <select value={tool} onChange={(e) => selectTool(e.target.value as DrawTool)}>
          <option value="select">Select (no draw)</option>
          <option value="Rectangle">Draw: Rectangle</option>
          <option value="Line">Draw: Line</option>
          <option value="Arrow">Draw: Arrow</option>
          <option value="Cloud">Draw: Cloud (click points, then Finish)</option>
          <option value="Text">Draw: Text (click to place)</option>
        </select>
        <input type="color" value={color} onChange={(e) => setColor(e.target.value)} />
        {tool === "Cloud" && (
          <button onClick={finishCloud} disabled={cloudPoints.length < 3}>
            Finish cloud ({cloudPoints.length} pts)
          </button>
        )}
      </div>
      <div className="pdf-canvas-wrap" style={{ width: RENDER_WIDTH, height: renderedHeight }}>
        {imageUri ? (
          <img src={imageUri} width={RENDER_WIDTH} height={renderedHeight} draggable={false} alt={`page ${page.page_number}`} />
        ) : (
          <div className="thumb-placeholder" style={{ width: RENDER_WIDTH, height: renderedHeight }}>
            rendering…
          </div>
        )}
        <svg
          ref={svgRef}
          width={RENDER_WIDTH}
          height={renderedHeight}
          className="pdf-canvas-overlay"
          style={{ cursor: moveState ? "grabbing" : resizeState ? "nwse-resize" : tool === "select" ? "default" : "crosshair" }}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
        >
          <defs>
            <marker id={`arrowhead-${page.id}`} markerWidth="8" markerHeight="8" refX="6" refY="4" orient="auto">
              <path d="M0,0 L8,4 L0,8 Z" fill="context-stroke" />
            </marker>
          </defs>
          {markups
            .filter((m) => !m.hidden)
            .map((m) => {
              const dragging = (moveState?.id === m.id || resizeState?.id === m.id) && livePoints;
              const shown = dragging ? { ...m, geometry: { ...m.geometry, points: livePoints! } } : m;
              return <MarkupShape key={m.id} markup={shown} toPixel={toPixel} arrowMarkerId={`arrowhead-${page.id}`} />;
            })}
          {tool === "select" &&
            selectedId &&
            (() => {
              const sel = markups.find((m) => m.id === selectedId);
              if (!sel) return null;
              const dragging = (moveState?.id === sel.id || resizeState?.id === sel.id) && livePoints;
              const points = dragging ? livePoints! : sel.geometry.points;
              const box = boundingBox(points);
              const [bx1, by1] = toPixel([box.minX, box.minY]);
              const [bx2, by2] = toPixel([box.maxX, box.maxY]);
              const handles = resizeHandles({ ...sel, geometry: { ...sel.geometry, points } });
              return (
                <g pointerEvents="none">
                  <rect
                    x={Math.min(bx1, bx2) - 4}
                    y={Math.min(by1, by2) - 4}
                    width={Math.abs(bx2 - bx1) + 8}
                    height={Math.abs(by2 - by1) + 8}
                    fill="none"
                    stroke="#2196f3"
                    strokeWidth={1}
                    strokeDasharray="3 2"
                  />
                  {handles.map((h, i) => {
                    const [hx, hy] = toPixel(h);
                    return <circle key={i} cx={hx} cy={hy} r={5} fill="#2196f3" />;
                  })}
                </g>
              );
            })()}
          {isDragTool && dragStart && dragCurrent && (
            <PreviewShape type={tool as MarkupType} start={dragStart} current={dragCurrent} toPixel={toPixel} color={color} />
          )}
          {cloudPoints.length > 0 && (
            <polyline
              points={cloudPoints.map((p) => toPixel(p).join(",")).join(" ")}
              fill="none"
              stroke={color}
              strokeWidth={2}
              strokeDasharray="4 2"
            />
          )}
        </svg>
        {pendingTextPoint && (
          <input
            autoFocus
            className="canvas-text-input"
            style={{ left: toPixel(pendingTextPoint)[0], top: toPixel(pendingTextPoint)[1] }}
            value={pendingTextValue}
            onChange={(e) => setPendingTextValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitPendingText();
              if (e.key === "Escape") setPendingTextPoint(null);
            }}
            onBlur={commitPendingText}
          />
        )}
      </div>
    </div>
  );
}

function MarkupShape({
  markup,
  toPixel,
  arrowMarkerId,
}: {
  markup: MarkupDto;
  toPixel: (p: Point) => Point;
  arrowMarkerId: string;
}) {
  const pts = markup.geometry.points.map(toPixel);
  const color = markup.style.color ?? "#e53935";
  const strokeWidth = markup.style.stroke_width ?? 2;

  switch (markup.markup_type) {
    case "Rectangle": {
      const [[x1, y1], [x2, y2]] = pts;
      return (
        <rect
          x={Math.min(x1, x2)}
          y={Math.min(y1, y2)}
          width={Math.abs(x2 - x1)}
          height={Math.abs(y2 - y1)}
          fill="none"
          stroke={color}
          strokeWidth={strokeWidth}
        />
      );
    }
    case "Line":
    case "Arrow": {
      const [[x1, y1], [x2, y2]] = pts;
      return (
        <line
          x1={x1}
          y1={y1}
          x2={x2}
          y2={y2}
          stroke={color}
          strokeWidth={strokeWidth}
          markerEnd={markup.markup_type === "Arrow" ? `url(#${arrowMarkerId})` : undefined}
        />
      );
    }
    case "Cloud":
      return <polygon points={pts.map((p) => p.join(",")).join(" ")} fill={`${color}22`} stroke={color} strokeWidth={strokeWidth} />;
    case "Text": {
      const [x, y] = pts[0] ?? [0, 0];
      return (
        <text x={x} y={y} fill={color} fontSize={14} fontFamily="inherit">
          {markup.style.text || "(text)"}
        </text>
      );
    }
    default:
      return null;
  }
}

function PreviewShape({
  type,
  start,
  current,
  toPixel,
  color,
}: {
  type: MarkupType;
  start: Point;
  current: Point;
  toPixel: (p: Point) => Point;
  color: string;
}) {
  const [x1, y1] = toPixel(start);
  const [x2, y2] = toPixel(current);
  if (type === "Rectangle") {
    return (
      <rect
        x={Math.min(x1, x2)}
        y={Math.min(y1, y2)}
        width={Math.abs(x2 - x1)}
        height={Math.abs(y2 - y1)}
        fill="none"
        stroke={color}
        strokeWidth={2}
        strokeDasharray="4 2"
      />
    );
  }
  return <line x1={x1} y1={y1} x2={x2} y2={y2} stroke={color} strokeWidth={2} strokeDasharray="4 2" />;
}

// ---------------------------------------------------------------------------
// Markup + Measurement, scoped to one page (MARK-*/MEAS-*)
// ---------------------------------------------------------------------------

function PagePanel({
  page,
  user,
  runAction,
}: {
  page: PageDto;
  user: UserDto;
  runAction: (fn: () => Promise<void>) => Promise<void>;
}) {
  // -- markup --
  const [markups, setMarkups] = useState<MarkupDto[]>([]);
  const [commentsByMarkup, setCommentsByMarkup] = useState<Record<string, MarkupCommentDto[]>>({});
  const [commentDraft, setCommentDraft] = useState<Record<string, string>>({});

  const reloadMarkups = () => runAction(async () => setMarkups(await api.listMarkupsByPage(page.id)));

  useEffect(() => {
    reloadMarkups();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page.id]);

  const toggleLock = (m: MarkupDto) => runAction(async () => {
    await api.setMarkupLocked(m.id, !m.locked);
    await reloadMarkups();
  });

  const toggleHidden = (m: MarkupDto) => runAction(async () => {
    await api.setMarkupHidden(m.id, !m.hidden);
    await reloadMarkups();
  });

  const removeMarkup = (m: MarkupDto) => runAction(async () => {
    await api.deleteMarkup(m.id);
    await reloadMarkups();
  });

  const loadComments = (markupId: string) =>
    runAction(async () => {
      const comments = await api.listMarkupComments(markupId);
      setCommentsByMarkup((prev) => ({ ...prev, [markupId]: comments }));
    });

  const addComment = (markupId: string) =>
    runAction(async () => {
      const text = commentDraft[markupId];
      if (!text) return;
      await api.addMarkupComment(markupId, user.id, text);
      setCommentDraft((prev) => ({ ...prev, [markupId]: "" }));
      await loadComments(markupId);
    });

  // -- measurement --
  const [scale, setScale] = useState<ScaleDto | null>(null);
  const [calP1, setCalP1] = useState("0,0");
  const [calP2, setCalP2] = useState("2,0");
  const [calInches, setCalInches] = useState("120");
  const [measurements, setMeasurements] = useState<MeasurementDto[]>([]);
  const [lenP1, setLenP1] = useState("0,0");
  const [lenP2, setLenP2] = useState("2,0");
  const [lenUnit, setLenUnit] = useState<api.LengthUnitCode>("ft");
  const [countMarkers, setCountMarkers] = useState("1,1 2,2 3,3");

  const reloadMeasurements = () =>
    runAction(async () => setMeasurements(await api.listMeasurementsByPage(page.id)));

  useEffect(() => {
    runAction(async () => setScale(await api.latestScaleForPage(page.id)));
    reloadMeasurements();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page.id]);

  const calibrate = () =>
    runAction(async () => {
      const s = await api.calibrateScale(page.id, parsePoint(calP1), parsePoint(calP2), Number(calInches), "imperial", user.id);
      setScale(s);
    });

  const recordLength = () =>
    runAction(async () => {
      if (!scale) throw new Error("calibrate a scale first");
      await api.recordLength(page.id, scale.id, parsePoint(lenP1), parsePoint(lenP2), lenUnit, null);
      await reloadMeasurements();
    });

  const recordCount = () =>
    runAction(async () => {
      await api.recordCount(page.id, parsePoints(countMarkers), null);
      await reloadMeasurements();
    });

  const removeMeasurement = (m: MeasurementDto) =>
    runAction(async () => {
      await api.deleteMeasurement(m.id);
      await reloadMeasurements();
    });

  return (
    <div className="nested">
      <h3>
        Markup + Measurement — page {page.page_number} ({page.width.toFixed(0)}×{page.height.toFixed(0)} pt)
      </h3>

      <div className="two-col">
        <div>
          <h4>Markup (MARK-01–04/07/08)</h4>
          <PdfCanvas page={page} markups={markups} user={user} onCreated={reloadMarkups} runAction={runAction} />
          <ul>
            {markups.map((m) => (
              <li key={m.id}>
                <div>
                  <strong>{m.markup_type}</strong> {JSON.stringify(m.geometry.points)}{" "}
                  {m.locked && <span className="tag">locked</span>} {m.hidden && <span className="tag">hidden</span>}
                </div>
                <div className="row">
                  <button onClick={() => toggleLock(m)}>{m.locked ? "unlock" : "lock"}</button>
                  <button onClick={() => toggleHidden(m)}>{m.hidden ? "show" : "hide"}</button>
                  <button onClick={() => removeMarkup(m)}>delete</button>
                  <button onClick={() => loadComments(m.id)}>comments ({commentsByMarkup[m.id]?.length ?? "?"})</button>
                </div>
                {commentsByMarkup[m.id] && (
                  <div className="nested">
                    <ul>
                      {commentsByMarkup[m.id].map((c) => (
                        <li key={c.id}>{c.text}</li>
                      ))}
                    </ul>
                    <div className="row">
                      <input
                        placeholder="add a comment"
                        value={commentDraft[m.id] ?? ""}
                        onChange={(e) => setCommentDraft((prev) => ({ ...prev, [m.id]: e.target.value }))}
                      />
                      <button onClick={() => addComment(m.id)}>Comment</button>
                    </div>
                  </div>
                )}
              </li>
            ))}
          </ul>
        </div>

        <div>
          <h4>Measurement (MEAS-01–07)</h4>
          <div className="row">
            <input placeholder="p1 x,y" value={calP1} onChange={(e) => setCalP1(e.target.value)} />
            <input placeholder="p2 x,y" value={calP2} onChange={(e) => setCalP2(e.target.value)} />
            <input placeholder="real-world inches" value={calInches} onChange={(e) => setCalInches(e.target.value)} />
            <button onClick={calibrate}>Calibrate scale</button>
          </div>
          {scale && (
            <p className="muted">
              Scale: {scale.inches_per_page_unit.toFixed(4)} in/page-unit ({scale.unit_system})
            </p>
          )}

          <div className="row">
            <input placeholder="p1 x,y" value={lenP1} onChange={(e) => setLenP1(e.target.value)} />
            <input placeholder="p2 x,y" value={lenP2} onChange={(e) => setLenP2(e.target.value)} />
            <select value={lenUnit} onChange={(e) => setLenUnit(e.target.value as api.LengthUnitCode)}>
              <option value="in">in</option>
              <option value="ft">ft</option>
              <option value="mm">mm</option>
              <option value="cm">cm</option>
              <option value="m">m</option>
            </select>
            <button onClick={recordLength} disabled={!scale}>
              Record length
            </button>
          </div>

          <div className="row">
            <input placeholder="markers: x,y x,y ..." value={countMarkers} onChange={(e) => setCountMarkers(e.target.value)} />
            <button onClick={recordCount}>Record count</button>
          </div>

          <ul>
            {measurements.map((m) => (
              <li key={m.id}>
                {m.measurement_type}: {m.value.toFixed(3)} {m.unit} {m.label && `— ${m.label}`}{" "}
                <button onClick={() => removeMeasurement(m)}>delete</button>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Takeoff (TAKE-01–05), scoped to the whole document
// ---------------------------------------------------------------------------

function TakeoffPanel({
  document: doc,
  runAction,
}: {
  document: DocumentDto;
  runAction: (fn: () => Promise<void>) => Promise<void>;
}) {
  const [items, setItems] = useState<TakeoffItemDto[]>([]);
  const [description, setDescription] = useState("");
  const [quantity, setQuantity] = useState("1");
  const [unit, setUnit] = useState("ea");
  const [costPerUnit, setCostPerUnit] = useState("");
  const [csv, setCsv] = useState<string | null>(null);

  const reload = () => runAction(async () => setItems(await api.listTakeoffForDocument(doc.id)));

  useEffect(() => {
    reload();
    setCsv(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc.id]);

  const createItem = () =>
    runAction(async () => {
      await api.createTakeoffItem(
        description,
        Number(quantity),
        unit,
        null,
        costPerUnit ? Number(costPerUnit) : null,
        null,
      );
      setDescription("");
      await reload();
    });

  const removeItem = (id: string) =>
    runAction(async () => {
      await api.deleteTakeoffItem(id);
      await reload();
    });

  const exportCsv = () => runAction(async () => setCsv(await api.exportTakeoffCsv(doc.id)));

  return (
    <div className="nested">
      <h3>Takeoff (TAKE-01–05)</h3>
      <div className="row">
        <input placeholder="description" value={description} onChange={(e) => setDescription(e.target.value)} />
        <input placeholder="quantity" value={quantity} onChange={(e) => setQuantity(e.target.value)} />
        <input placeholder="unit" value={unit} onChange={(e) => setUnit(e.target.value)} />
        <input placeholder="cost/unit (optional)" value={costPerUnit} onChange={(e) => setCostPerUnit(e.target.value)} />
        <button onClick={createItem} disabled={!description}>
          Add line item
        </button>
      </div>
      <ul>
        {items.map((item) => (
          <li key={item.id}>
            {item.description} — {item.quantity} {item.unit}
            {item.total_cost != null && ` — $${item.total_cost.toFixed(2)}`}{" "}
            <button onClick={() => removeItem(item.id)}>delete</button>
          </li>
        ))}
      </ul>
      <button onClick={exportCsv}>Export CSV</button>
      {csv && <pre className="csv-preview">{csv}</pre>}
    </div>
  );
}
