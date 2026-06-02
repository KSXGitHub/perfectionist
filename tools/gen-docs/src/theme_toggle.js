// ============================================================================
// Theme (colour-scheme) settings.
//
// Drives the gear button and the Settings panel it toggles, and applies
// the reader's saved colour-scheme choice. The page's colours live in
// style/light.css and style/dark.css, selected across three override
// tiers (see the comments in those files):
//
//   1. Default: Light.
//   2. System preference: `@media (prefers-color-scheme: dark)` swaps in
//      the dark palette.
//   3. Explicit override: a `color-scheme-override="dark"|"light"`
//      attribute on <html>, which beats both tiers above. ONLY this
//      script sets it.
//
// This file's job, in order:
//
//   1. Assign the event listeners (gear toggle, the three theme radios,
//      outside-click / Escape to dismiss).
//   2. Reveal the gear button (the Rust template emits it with the HTML
//      `hidden` attribute; clearing it is how the button appears).
//   3. Read `perfectionist-color-scheme-override` from localStorage and
//      apply it.
//
// If step 1 throws — the likely cause being a browser too old for the
// APIs used — execution stops before steps 2 and 3, so the gear stays
// hidden rather than appearing as a dead control. This is the same
// "reveal only once wired up" contract the nav hamburger uses, and the
// absence of a try/catch is deliberate: a half-supported browser should
// get the plain Light/System-preference page, not a visible-but-inert
// settings UI. (The `[hidden] { display: none !important }` reset in
// style/base.css keeps the `hidden` attribute authoritative against the
// `.settings-toggle { display: flex }` rule.)
//
// The localStorage key is `perfectionist-color-scheme-override` — long
// on purpose, because `gh-pages/index.html` is also opened over
// `file:///`, where the origin is shared across every local HTML file
// and a short key would risk collisions.
// ============================================================================

(function () {
  var html = document.documentElement;
  var STORAGE_KEY = "perfectionist-color-scheme-override";
  var ATTRIBUTE = "color-scheme-override";

  var toggle = document.querySelector(".settings-toggle");
  var panel = document.querySelector(".settings-panel");
  if (!toggle || !panel) return;

  var radios = panel.querySelectorAll('input[name="color-scheme"]');
  if (radios.length === 0) return;

  // Apply a choice to the live page. "dark"/"light" set the override
  // attribute (tier 3); anything else ("system") removes it so the
  // page falls back to tiers 1–2.
  function applyScheme(value) {
    if (value === "dark" || value === "light") {
      html.setAttribute(ATTRIBUTE, value);
    } else {
      html.removeAttribute(ATTRIBUTE);
    }
  }

  // Persist a choice. "system" clears the key (its absence IS the
  // default), keeping the stored set to exactly {dark, light, unset}.
  function persistScheme(value) {
    if (value === "dark" || value === "light") {
      window.localStorage.setItem(STORAGE_KEY, value);
    } else {
      window.localStorage.removeItem(STORAGE_KEY);
    }
  }

  function selectRadio(value) {
    for (var i = 0; i < radios.length; i++) {
      radios[i].checked = radios[i].value === value;
    }
  }

  function openPanel() {
    panel.hidden = false;
    toggle.setAttribute("aria-expanded", "true");
  }

  function closePanel() {
    panel.hidden = true;
    toggle.setAttribute("aria-expanded", "false");
  }

  // ---- (1) event listeners ----------------------------------------------

  toggle.addEventListener("click", function () {
    if (toggle.getAttribute("aria-expanded") === "true") {
      closePanel();
    } else {
      openPanel();
    }
  });

  for (var i = 0; i < radios.length; i++) {
    radios[i].addEventListener("change", function (event) {
      applyScheme(event.target.value);
      persistScheme(event.target.value);
    });
  }

  // Dismiss the panel on a click anywhere outside it (and outside the
  // gear, whose own handler already toggles). Registered on the
  // document so any stray click closes the dropdown, the conventional
  // behaviour for this kind of menu.
  document.addEventListener("click", function (event) {
    if (toggle.getAttribute("aria-expanded") !== "true") return;
    if (toggle.contains(event.target) || panel.contains(event.target)) return;
    closePanel();
  });

  // Escape closes the panel and returns focus to the gear.
  document.addEventListener("keydown", function (event) {
    if (event.key !== "Escape") return;
    if (toggle.getAttribute("aria-expanded") !== "true") return;
    closePanel();
    toggle.focus();
  });

  // Mirror the hamburger's Visual Viewport API compensation (see
  // nav_toggle.js): on mobile browsers that anchor `position: fixed` to
  // the layout viewport rather than the visual viewport, translate the
  // gear and its panel by the visual viewport's offset so they stay
  // glued to the top-right of the visible area as the URL bar collapses.
  if (window.visualViewport) {
    var vv = window.visualViewport;
    var syncToViewport = function () {
      var transform = "translate(" + vv.offsetLeft + "px, " + vv.offsetTop + "px)";
      toggle.style.transform = transform;
      panel.style.transform = transform;
    };
    vv.addEventListener("scroll", syncToViewport);
    vv.addEventListener("resize", syncToViewport);
    syncToViewport();
  }

  // ---- (2) reveal the gear ----------------------------------------------
  //
  // Everything the button needs is wired up; drop the `hidden` attribute
  // so it appears exactly when it's functional. If anything above threw,
  // this line is never reached and the gear stays hidden.
  toggle.hidden = false;

  // ---- (3) load + apply the persisted choice ----------------------------
  var stored = window.localStorage.getItem(STORAGE_KEY);
  if (stored === "dark" || stored === "light") {
    applyScheme(stored);
    selectRadio(stored);
  } else {
    applyScheme(null);
    selectRadio("system");
  }
})();
