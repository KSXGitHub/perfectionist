// ============================================================================
// Why this file is necessary at all (i.e., why CSS-only doesn't work).
//
// Mobile browsers have two independent "viewports":
//   * Layout viewport — the virtual canvas derived from
//     `<meta name="viewport" content="width=device-width">`. Its size does
//     NOT change with URL-bar dynamics, pinch-zoom, or the on-screen
//     keyboard.
//   * Visual viewport — what the user actually sees right now. Shrinks /
//     grows when the URL bar collapses, when you pinch-zoom, when the
//     keyboard opens.
//
// Per the modern CSS spec, `position: fixed` resolves against the visual
// viewport. But several Android browsers (confirmed on Galaxy M15 in
// Chrome AND Firefox) and certain device-specific WebView builds still
// anchor it to the LAYOUT viewport. The visual symptom is "the fixed
// hamburger scrolls away with the page when the URL bar collapses": as
// the visual viewport moves downward within the (static) layout
// viewport, a `position: fixed` element pinned to the layout viewport's
// top ends up above the visible area.
//
// CSS alone has no escape hatch:
//   * Nothing in CSS exposes which viewport `position: fixed` resolved
//     against, lets us override it, or queries the visual viewport's
//     offset.
//   * `overflow: hidden` on body/html doesn't even reliably stop
//     scrolling on iOS — a well-known cross-browser gap.
//
// So this script does three things CSS can't:
//
//   1. Body scroll lock while the drawer is open
//      (`body { position: fixed; top: -<scrollY>px }`, restored on
//      close). This is the load-bearing fix: with body unable to
//      scroll, the URL bar can't dynamically collapse, so every
//      `position: fixed` element stays glued to the visible area for
//      the duration of the menu interaction. Same pattern is the only
//      cross-browser way to stop background scroll on iOS.
//
//   2. Visual Viewport API offset compensation on the toggle while the
//      drawer is closed, so the hamburger stays at the top of the
//      visible area even on browsers that mis-anchor `position: fixed`.
//      `window.visualViewport.offsetTop/offsetLeft` are JS-only; CSS has
//      no equivalent.
//
//   3. Imperative open/close + scrollY snapshot. The CSS-only
//      `<details>`/`<summary>` toggle can't be closed in response to
//      other events (a sidebar link tap, a tap on the in-overlay close
//      button) and can't drive the scroll lock. It can also create a
//      containing block for fixed descendants on some mobile Firefox
//      builds, turning the supposedly-fixed `<summary>` into effective
//      `position: absolute` — eliminating `<details>` removes that
//      whole category of UA quirk too.
//
// `position: fixed` is still the right CSS primitive. This file doesn't
// replace it — it stabilises the viewport that `position: fixed`
// resolves against.
// ============================================================================

(function () {
  var toggle = document.querySelector(".nav-toggle");
  var sidebar = document.querySelector(".nav-sidebar");
  if (!toggle || !sidebar) return;

  // ---- Hamburger fade ---------------------------------------------------
  //
  // Hide the hamburger while the catalogue's <h1> is on screen. Default
  // is visible — if this branch never runs (no IntersectionObserver,
  // CSP-blocked script, etc.) the drawer remains reachable. The earlier
  // draft observed `table.index` instead, but on phone-height viewports
  // the table is taller than the viewport and stayed partially
  // intersecting through the entire articles section, leaving the
  // hamburger hidden long after the reader had left the index.
  var heading = document.querySelector("h1#catalogue");
  if (heading && "IntersectionObserver" in window) {
    var observer = new IntersectionObserver(function (entries) {
      var entry = entries[entries.length - 1];
      toggle.classList.toggle("nav-toggle-hidden", entry.isIntersecting);
    });
    observer.observe(heading);
  }

  // ---- Visual Viewport API offset compensation --------------------------
  //
  // See concern (2) in the file header: translate the toggle by the
  // visual viewport's offset so it stays glued to the visible area on
  // browsers that anchor `position: fixed` to the layout viewport.
  if (window.visualViewport) {
    var vv = window.visualViewport;
    var syncToggleToViewport = function () {
      toggle.style.transform =
        "translate(" + vv.offsetLeft + "px, " + vv.offsetTop + "px)";
    };
    vv.addEventListener("scroll", syncToggleToViewport);
    vv.addEventListener("resize", syncToggleToViewport);
    syncToggleToViewport();
  }

  // ---- Open / close + body scroll lock ----------------------------------
  //
  // See concern (1) in the file header for the load-bearing role of the
  // scroll lock. The drawer is opened by tapping `.nav-toggle` and
  // closed by tapping `.nav-sidebar-close` (the ✕ inside the overlay).
  // Body scroll lock has the side benefit of stopping the page behind
  // from scrolling when the user swipes within the overlay. We preserve
  // the scroll position by snapping body to `top: -<y>px` while locked
  // and restoring `scrollTo(0, y)` on unlock.
  var savedScrollY = 0;
  var bodyLocked = false;

  function lockBodyScroll() {
    if (bodyLocked) return;
    savedScrollY = window.scrollY;
    document.body.style.position = "fixed";
    document.body.style.top = "-" + savedScrollY + "px";
    document.body.style.left = "0";
    document.body.style.right = "0";
    bodyLocked = true;
  }

  function unlockBodyScroll() {
    if (!bodyLocked) return;
    document.body.style.position = "";
    document.body.style.top = "";
    document.body.style.left = "";
    document.body.style.right = "";
    window.scrollTo(0, savedScrollY);
    bodyLocked = false;
  }

  function openSidebar() {
    toggle.setAttribute("aria-expanded", "true");
    lockBodyScroll();
  }

  function closeSidebar() {
    toggle.setAttribute("aria-expanded", "false");
    unlockBodyScroll();
  }

  toggle.addEventListener("click", function () {
    if (toggle.getAttribute("aria-expanded") === "true") {
      closeSidebar();
    } else {
      openSidebar();
    }
  });

  var closeBtn = sidebar.querySelector(".nav-sidebar-close");
  if (closeBtn) closeBtn.addEventListener("click", closeSidebar);

  // ---- Close on sidebar-link follow -------------------------------------
  //
  // Tapping a rule link in the open overlay should navigate to the rule
  // AND close the overlay so the rule is visible. Modifier-key clicks
  // (Ctrl/Cmd/Shift/Alt) and non-primary buttons are standard "open in
  // new tab/window" gestures and must leave the current page intact.
  // Also move focus to the destination so keyboard users don't lose
  // their place: the just-clicked <a> is inside a now-`display: none`
  // sidebar and focus would otherwise drop to <body>. Rule articles and
  // the catalogue heading aren't focusable by default, so set
  // tabindex="-1" before .focus().
  sidebar.addEventListener("click", function (event) {
    var link = event.target.closest("a");
    if (!link) return;
    if (event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    closeSidebar();
    var hash = link.hash;
    if (!hash) return;
    var target;
    try {
      target = document.querySelector(hash);
    } catch (_e) {
      return;
    }
    if (!target) return;
    target.setAttribute("tabindex", "-1");
    target.focus({ preventScroll: true });
  });
})();
  var savedScrollY = 0;
  var bodyLocked = false;

  function lockBodyScroll() {
    if (bodyLocked) return;
    savedScrollY = window.scrollY;
    document.body.style.position = "fixed";
    document.body.style.top = "-" + savedScrollY + "px";
    document.body.style.left = "0";
    document.body.style.right = "0";
    bodyLocked = true;
  }

  function unlockBodyScroll() {
    if (!bodyLocked) return;
    document.body.style.position = "";
    document.body.style.top = "";
    document.body.style.left = "";
    document.body.style.right = "";
    window.scrollTo(0, savedScrollY);
    bodyLocked = false;
  }

  function openSidebar() {
    toggle.setAttribute("aria-expanded", "true");
    lockBodyScroll();
  }

  function closeSidebar() {
    toggle.setAttribute("aria-expanded", "false");
    unlockBodyScroll();
  }

  toggle.addEventListener("click", function () {
    if (toggle.getAttribute("aria-expanded") === "true") {
      closeSidebar();
    } else {
      openSidebar();
    }
  });

  var closeBtn = sidebar.querySelector(".nav-sidebar-close");
  if (closeBtn) closeBtn.addEventListener("click", closeSidebar);

  // ---- Close on sidebar-link follow -------------------------------------
  //
  // Tapping a rule link in the open overlay should navigate to the rule
  // AND close the overlay so the rule is visible. Modifier-key clicks
  // (Ctrl/Cmd/Shift/Alt) and non-primary buttons are standard "open in
  // new tab/window" gestures and must leave the current page intact.
  // Also move focus to the destination so keyboard users don't lose
  // their place: the just-clicked <a> is inside a now-`display: none`
  // sidebar and focus would otherwise drop to <body>. Rule articles and
  // the catalogue heading aren't focusable by default, so set
  // tabindex="-1" before .focus().
  sidebar.addEventListener("click", function (event) {
    var link = event.target.closest("a");
    if (!link) return;
    if (event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    closeSidebar();
    var hash = link.hash;
    if (!hash) return;
    var target;
    try {
      target = document.querySelector(hash);
    } catch (_e) {
      return;
    }
    if (!target) return;
    target.setAttribute("tabindex", "-1");
    target.focus({ preventScroll: true });
  });
})();
