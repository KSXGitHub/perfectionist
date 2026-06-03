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
// The buttons are stateless — they set or clear `open` and don't track or
// reflect the current open/closed mix. A stateless pair is the simplest cut
// and matches what the catalogue needs.
// ============================================================================

(function () {
  var section = document.querySelector(".config-controls");
  if (!section) return;

  var buttons = section.querySelectorAll("button[data-config-open]");
  if (buttons.length === 0) return;

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

  for (var i = 0; i < buttons.length; i++) {
    buttons[i].addEventListener("click", function (event) {
      setAllOpen(event.currentTarget.getAttribute("data-config-open") === "true");
    });
  }

  // Everything is wired up; reveal the section so the buttons appear exactly
  // when they work.
  section.hidden = false;
})();
