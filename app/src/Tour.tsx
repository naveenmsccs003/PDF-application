import { useEffect, useState } from "react";

// First-time user tour. Deliberately a plain sequence of description cards
// rather than DOM-anchored spotlighting on live elements — most of what it
// describes (projects, documents, the canvas, takeoff, RFIs) only exists
// conditionally after sign-in/project/document selection, so anchoring to
// real elements would mean either faking that state or leaving early steps
// with nothing to point at. A description-only walkthrough stays correct
// regardless of what's on screen right now.

export interface TourStep {
  title: string;
  body: string;
}

const TOUR_STEPS: TourStep[] = [
  {
    title: "Welcome to MDS Rebar",
    body:
      "One desktop app for the whole drawing-review workflow: open a PDF, mark it up, " +
      "measure it, turn those measurements into a takeoff, and hand off a flattened " +
      "package at the end. This short tour walks through each stage — about a minute.",
  },
  {
    title: "Sign in & projects",
    body:
      "Signing in just needs an email — a new one creates an account, an existing one " +
      "signs you back in. Projects are shared: add a teammate by email and they'll see " +
      "your markups, RFIs, and takeoff the next time they open the project.",
  },
  {
    title: "Import & view drawings",
    body:
      "Import PDF… brings a drawing set in page by page, browsable as thumbnails. On the " +
      "page view, Zoom (+/−/Reset) and the Pan tool work like any PDF viewer — nothing " +
      "here needs the markup tools to just look at a sheet.",
  },
  {
    title: "Markup & measure",
    body:
      "Click-drag to draw rectangles, clouds, lines, arrows, or text redlines straight on " +
      "the page. Calibrate a scale once per drawing, then Length/Area/Count measurements " +
      "read real-world units. Everything's select/move/resize-able, with undo/redo and a " +
      "layer list that stays in sync with what's on the canvas.",
  },
  {
    title: "Takeoff",
    body:
      "Turn a measurement into a quantity line item — description, unit, cost — linked " +
      "back to what you measured on the page. Export the whole list to CSV or Excel when " +
      "it's ready for estimating.",
  },
  {
    title: "RFIs & revisions",
    body:
      "File an RFI tied to a page or a specific markup and track it open → answered → " +
      "closed. Save a numbered drawing revision at any point, reopen an old one, or " +
      "compare two revisions side by side to see exactly what changed.",
  },
  {
    title: "Autosave, export & handoff",
    body:
      "Every change saves itself as you make it, plus a periodic snapshot you can restore " +
      "from if something goes wrong. When a drawing's ready to go out, Export flattened " +
      "PDF… burns your markups into the page, and Export handoff package… bundles that " +
      "with the takeoff CSV in one folder.",
  },
  {
    title: "You're set",
    body:
      "That's the whole loop: import, mark up, measure, take off, hand off. Click " +
      "\"Take the tour\" in the header any time to see this again.",
  },
];

const TOUR_SEEN_KEY = "mds-rebar:tour-seen-v1";

export function hasTourBeenSeen(): boolean {
  try {
    return localStorage.getItem(TOUR_SEEN_KEY) === "1";
  } catch {
    // Storage unavailable (private browsing, disabled site data): don't
    // force the tour open on every single load — worse than just not
    // remembering "seen" across sessions.
    return true;
  }
}

function markTourSeen() {
  try {
    localStorage.setItem(TOUR_SEEN_KEY, "1");
  } catch {
    // Nothing to recover — worst case the tour reappears next launch.
  }
}

export function ProductTour({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [stepIndex, setStepIndex] = useState(0);

  useEffect(() => {
    if (open) setStepIndex(0);
  }, [open]);

  const isFirst = stepIndex === 0;
  const isLast = stepIndex === TOUR_STEPS.length - 1;

  const finish = () => {
    markTourSeen();
    onClose();
  };
  const next = () => (isLast ? finish() : setStepIndex((i) => Math.min(TOUR_STEPS.length - 1, i + 1)));
  const back = () => setStepIndex((i) => Math.max(0, i - 1));

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") finish();
      else if (e.key === "ArrowRight") next();
      else if (e.key === "ArrowLeft") back();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, stepIndex]);

  if (!open) return null;

  const step = TOUR_STEPS[stepIndex];

  return (
    <div className="tour-backdrop" onClick={finish}>
      <div
        className="tour-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="tour-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="tour-header">
          <span className="tour-step-count">
            Step {stepIndex + 1} of {TOUR_STEPS.length}
          </span>
          <button className="tour-skip" onClick={finish}>
            Skip tour
          </button>
        </div>
        <h2 id="tour-title">{step.title}</h2>
        <p>{step.body}</p>
        <div className="tour-dots" aria-hidden="true">
          {TOUR_STEPS.map((_, i) => (
            <span key={i} className={i === stepIndex ? "tour-dot active" : "tour-dot"} />
          ))}
        </div>
        <div className="tour-actions">
          <button onClick={back} disabled={isFirst}>
            Back
          </button>
          <button className="tour-next" onClick={next}>
            {isLast ? "Get started" : "Next"}
          </button>
        </div>
      </div>
    </div>
  );
}
