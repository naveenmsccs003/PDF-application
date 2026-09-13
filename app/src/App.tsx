import { useEffect, useState } from "react";
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
        the Rust domain crates via Tauri commands. This is a functional control panel for
        exercising the backend, not the eventual PDF markup canvas (that needs
        rendering/zoom/pan UI work not done yet).
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
  const [markupType, setMarkupType] = useState<MarkupType>("Rectangle");
  const [pointsInput, setPointsInput] = useState("0,0 2,1");
  const [colorInput, setColorInput] = useState("#ff0000");
  const [commentsByMarkup, setCommentsByMarkup] = useState<Record<string, MarkupCommentDto[]>>({});
  const [commentDraft, setCommentDraft] = useState<Record<string, string>>({});

  const reloadMarkups = () => runAction(async () => setMarkups(await api.listMarkupsByPage(page.id)));

  useEffect(() => {
    reloadMarkups();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [page.id]);

  const createMarkup = () =>
    runAction(async () => {
      const points = parsePoints(pointsInput);
      await api.createMarkup(page.id, markupType, { points }, { color: colorInput, stroke_width: null, text: null }, user.id);
      await reloadMarkups();
    });

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
          <div className="row">
            <select value={markupType} onChange={(e) => setMarkupType(e.target.value as MarkupType)}>
              <option>Text</option>
              <option>Rectangle</option>
              <option>Cloud</option>
              <option>Line</option>
              <option>Arrow</option>
            </select>
            <input placeholder="points, e.g. 0,0 2,1" value={pointsInput} onChange={(e) => setPointsInput(e.target.value)} />
            <input type="color" value={colorInput} onChange={(e) => setColorInput(e.target.value)} />
            <button onClick={createMarkup}>Add markup</button>
          </div>
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
