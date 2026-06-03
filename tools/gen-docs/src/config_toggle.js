// ============================================================================
// Configuration "Expand all" / "Collapse all" controls.
//
// The Settings panel's Configuration section holds two buttons that open or
// close every rule's Configuration <details> (details.config-details) at
// once. Toggling every <details> from one control has no pure-CSS
// expression, so this is the only part of the Settings panel that genuinely
// needs script.
//
// The Rust template (see config_controls() in render.rs) emits the whole
// section with the HTML `hidden` attribute; this file reveals it only after
// the click handlers are wired up — the same "reveal once functional"
// contract the gear and hamburger follow (the
// `[hidden] { display: none !important }` reset in style/base.css keeps
// `hidden` authoritative against the section's own `display`). A browser too
// old for the APIs used throws before the reveal line, so the section stays
// hidden rather than showing two dead buttons. Hiding the whole <fieldset>
// (not just the buttons) also keeps the "Configuration" legend from showing
// alone above an empty row.
//
// Kept separate from theme_toggle.js so the two Settings sections degrade
// independently: a failure in one script leaves its controls hidden without
// taking the other's down.
//
// The buttons also reflect state: "Expand all" is highlighted while every
// panel is open, "Collapse all" while every panel is closed, neither in a
// mixed state. The highlight rides on `aria-pressed` — both the
// accessibility signal (the pair become toggle buttons) and the CSS hook the
// colour layer keys off. State is kept live by listening for the <details>
// `toggle` event, which the browser fires whenever a panel's open state
// changes (user click, the bulk buttons here, or find-in-page auto-expand).
//
// `toggle` does not bubble, so a single CAPTURING listener on the document
// covers every panel — the capture phase reaches non-bubbling events from
// descendants. And because the browser fires one (asynchronous, per-element)
// `toggle` event per panel, a bulk action over N panels fires N events;
// rather than recompute N times, each event just requests a single recompute
// on the next animation frame, coalescing the burst into one pass.
// ============================================================================

(function () {
  var section = document.querySelector(".config-controls");
  if (!section) return;

  var expandButton = section.querySelector('button[data-config-open="true"]');
  var collapseButton = section.querySelector('button[data-config-open="false"]');
  if (!expandButton || !collapseButton) return;

  // Nothing to toggle means nothing to reveal: a catalogue with no
  // configurable rule renders no `details.config-details`, so leaving the
  // section hidden avoids two buttons that would silently do nothing.
  if (document.querySelectorAll("details.config-details").length === 0) return;

  function setAllOpen(open) {
    var panels = document.querySelectorAll("details.config-details");
    for (var i = 0; i < panels.length; i++) {
      panels[i].open = open;
    }
  }

  // Reflect the current open/closed mix onto the two buttons. A single pass
  // tracks whether every panel is open and whether every panel is closed —
  // they can't both be true once there's at least one panel, and both are
  // false in a mixed state, so neither button highlights then.
  function reflectState() {
    var panels = document.querySelectorAll("details.config-details");
    var allOpen = true;
    var allClosed = true;
    for (var i = 0; i < panels.length; i++) {
      if (panels[i].open) {
        allClosed = false;
      } else {
        allOpen = false;
      }
    }
    expandButton.setAttribute("aria-pressed", String(allOpen));
    collapseButton.setAttribute("aria-pressed", String(allClosed));
  }

  // Coalesce a burst of `toggle` events into one recompute per frame: the
  // first event in a frame queues the pass, the rest are no-ops until it
  // runs. This keeps "Expand all" / "Collapse all" — which fire one event
  // per panel — at a single pass instead of one per panel.
  var frame = 0;
  function runReflect() {
    frame = 0;
    reflectState();
  }
  function scheduleReflect() {
    if (frame) return;
    frame = window.requestAnimationFrame(runReflect);
  }

  expandButton.addEventListener("click", function () {
    setAllOpen(true);
  });
  collapseButton.addEventListener("click", function () {
    setAllOpen(false);
  });

  // One capturing listener for the non-bubbling `toggle` event, filtered to
  // the Configuration panels so unrelated <details> (if any are ever added)
  // don't drive the recompute.
  document.addEventListener(
    "toggle",
    function (event) {
      var target = event.target;
      if (target instanceof Element && target.matches("details.config-details")) {
        scheduleReflect();
      }
    },
    true,
  );

  // Everything is wired up; reveal the section so the buttons appear exactly
  // when they work.
  section.hidden = false;

  // Phase 1: a non-blocking initial pass so the buttons reflect whatever
  // state the page loads in (all collapsed by default, but the browser may
  // restore open panels via bfcache or open one for a fragment target).
  scheduleReflect();
})();
