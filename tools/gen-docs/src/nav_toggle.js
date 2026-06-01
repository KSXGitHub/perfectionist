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
// So this script does work that falls in three categories:
//
// A. Workarounds for mobile-browser `position: fixed` bugs.
//    These items only exist because some browsers anchor
//    `position: fixed` to the layout viewport rather than the
//    visual viewport. If `position: fixed` worked correctly on
//    every browser, we wouldn't need either of them.
//
//   1. Body scroll lock while the drawer is open
//      (`body { position: fixed; top: -<scrollY>px }`, restored on
//      close). With body unable to scroll, the URL bar can't
//      dynamically collapse, so every `position: fixed` element
//      stays glued to the visible area for the duration of the
//      menu interaction. (Bonus: same pattern is also the only
//      cross-browser way to stop background scroll behind a modal
//      on iOS — `overflow: hidden` is famously broken there, so
//      this is two CSS bugs avoided for the price of one
//      workaround.)
//
//   2. Visual Viewport API offset compensation on the toggle while
//      the drawer is closed. Same root cause — translate the
//      toggle by `visualViewport.offsetTop`/`offsetLeft` so it
//      stays at the top of the visible area on browsers that
//      mis-anchor `position: fixed`.
//      `window.visualViewport.offsetTop/offsetLeft` are JS-only;
//      CSS has no equivalent.
//
// B. Behaviours CSS genuinely doesn't provide. These would still
//    need code even on a hypothetical perfect-CSS browser.
//
//   3. Imperative open/close from arbitrary events. The CSS-only
//      `<details>`/`<summary>` toggle can't be closed in response
//      to a sidebar link tap or a separate close-button tap; CSS
//      has no event-handler equivalent. (Eliminating `<details>`
//      also avoids a separate Firefox-Mobile containing-block
//      quirk that turned the supposedly-fixed `<summary>` into
//      effective `position: absolute`, but that's a happy side
//      effect of switching to a `<button>`, not the reason for
//      the switch.)
//
//   4. Background inertness via the `inert` HTML attribute on
//      every direct body child except the toggle and sidebar while
//      the drawer is open. CSS has no equivalent for removing an
//      element from the focus order *and* AT exposure in one step.
//      `inert` is Baseline 2023, so older browsers silently ignore
//      it and would let Tab escape into obscured content; a small
//      keydown-based focus trap (also in this script) wraps Tab
//      inside the sidebar while the drawer is open as a fallback.
//      AT-exposure suppression on those older browsers degrades to
//      what we get for free from the static index table near the
//      top of the page — i.e. the page still has a navigation
//      affordance, just not a modal-isolated one.
//
//   5. Focus management on open and close. Opening the drawer
//      hides the toggle (`aria-expanded="true"` -> `display: none`)
//      and closing hides the close button the same way; CSS has no
//      primitive for moving focus when an element disappears, so a
//      keyboard user would otherwise lose focus to <body>.
//      Explicitly move focus to the close button on open and back
//      to the hamburger on close.
//
//   6. Hamburger fade tied to a sentinel element on the page.
//      Hide the toggle while the catalogue's <h1> is on screen and
//      fade it in once the reader scrolls past. CSS scroll-driven
//      animations (`view-timeline` / `animation-timeline`) could
//      do this declaratively, but they're not yet available across
//      the project's reader baseline. The IntersectionObserver
//      fallback is a UX nicety, not load-bearing — if it doesn't
//      run, the hamburger just stays visible throughout, which is
//      harmless.
//
//   7. Force-close on cross-breakpoint resize. CSS media queries
//      are stateless: there's no way for the `>=1100px` desktop
//      rule to "clean up" the open drawer's body scroll lock and
//      `inert` background when the viewport crosses the boundary.
//      `matchMedia` plus an event listener is the JS equivalent.
//      Defensive cleanup: only matters during rotation / window-
//      resize while the drawer happens to be open.
//
// `position: fixed` is still the right CSS primitive. This file
// doesn't replace it — it stabilises the viewport that
// `position: fixed` resolves against (category A) and implements
// the behaviours CSS doesn't reach (category B).
//
// C. The consequence of needing JS at all. Because A and B mean
//    the button only works when this script runs successfully, we
//    also have to hide the button when it *doesn't* — otherwise a
//    CSP-blocked inline script, a stripped script tag, or a parse
//    error before the handlers attach all leave a visible-but-
//    inert hamburger on the page. The Rust template renders the
//    button with the HTML `hidden` attribute and this script
//    clears it at the very end of setup; if anything throws before
//    then, the toggle stays hidden. `<noscript>` only catches
//    "scripting disabled in the browser", which is why we use
//    `hidden` instead. (One CSS subtlety: style/base.css ships an
//    author-side `[hidden] { display: none !important }` reset;
//    without it the `.nav-toggle { display: flex }` rule (in
//    style/nav.css) silently overrides the UA `[hidden]` rule and
//    the toggle stays visible despite `hidden`.)
// ============================================================================

(function () {
  var toggle = document.querySelector(".nav-toggle");
  var sidebar = document.querySelector(".nav-sidebar");
  if (!toggle || !sidebar) return;

  // ---- Hamburger fade ---------------------------------------------------
  //
  // Hide the hamburger while the catalogue's <h1> is on screen. Default
  // is visible — if this branch never runs (browser lacks
  // IntersectionObserver) the hamburger just stays revealed throughout,
  // which is harmless. (If the WHOLE script doesn't run — CSP block,
  // stripped tag, parse error — the `hidden` attribute the Rust
  // template emits keeps the button invisible; that case is category
  // C in the file header, not this branch's responsibility.) The earlier
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
  // See category A item 2 in the file header: translate the toggle by
  // the visual viewport's offset so it stays glued to the visible area
  // on browsers that anchor `position: fixed` to the layout viewport.
  // The same compensation has to apply to the sidebar — if the user
  // scrolled before tapping the toggle, `visualViewport.offsetTop` is
  // already non-zero by the time the drawer opens, and the
  // `position: fixed; top: 0` sidebar would render above the visible
  // viewport with its header (and close button) clipped off. Locking
  // body scroll on open only stops *further* URL-bar movement; an
  // existing offset stays put unless we explicitly compensate. Set
  // the height explicitly too, since `100dvh` is unreliable on the
  // same browsers that mis-anchor `position: fixed`.
  if (window.visualViewport) {
    var vv = window.visualViewport;
    var syncToViewport = function () {
      var t = "translate(" + vv.offsetLeft + "px, " + vv.offsetTop + "px)";
      toggle.style.transform = t;
      sidebar.style.transform = t;
      sidebar.style.height = vv.height + "px";
    };
    vv.addEventListener("scroll", syncToViewport);
    vv.addEventListener("resize", syncToViewport);
    syncToViewport();
  }

  // ---- Open / close + body scroll lock ----------------------------------
  //
  // See category A item 1 in the file header for the load-bearing role
  // of the scroll lock. The drawer is opened by tapping `.nav-toggle` and
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

  // ---- Background inertness ---------------------------------------------
  //
  // The overlay covers the page content (fully on phone-portrait, as a
  // left strip on phone-landscape / tablet). Without intervention, Tab
  // navigation could still reach the obscured content behind it, and
  // assistive tech could still read it — both surprising for users who
  // see a modal-style drawer. Apply the `inert` attribute (HTML
  // standard, Baseline 2023) to every direct child of <body> except
  // the toggle and the sidebar while the drawer is open; restore on
  // close. `inert` removes focusability and AT exposure of the
  // subtree without needing a manual focus trap or aria-hidden dance.
  var inertedChildren = [];
  function setBackgroundInert() {
    inertedChildren = [];
    for (var i = 0; i < document.body.children.length; i++) {
      var el = document.body.children[i];
      if (el === toggle || el === sidebar) continue;
      if (el.inert) continue;
      el.inert = true;
      inertedChildren.push(el);
    }
  }
  function clearBackgroundInert() {
    for (var i = 0; i < inertedChildren.length; i++) inertedChildren[i].inert = false;
    inertedChildren = [];
  }

  // ---- Focus management on open / close ---------------------------------
  //
  // Opening the drawer hides the toggle (`aria-expanded="true"` ->
  // `display: none`), so a keyboard user who just tabbed-and-Entered on
  // the hamburger would lose focus to <body>. Closing the drawer hides
  // the close button the same way. In both cases we explicitly move
  // focus to a sensible visible target so AT users keep their place.
  function openSidebar() {
    toggle.setAttribute("aria-expanded", "true");
    lockBodyScroll();
    setBackgroundInert();
    if (closeBtn) closeBtn.focus({ preventScroll: true });
  }

  function closeSidebar() {
    toggle.setAttribute("aria-expanded", "false");
    unlockBodyScroll();
    clearBackgroundInert();
    toggle.focus({ preventScroll: true });
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

  // ---- Focus trap fallback ----------------------------------------------
  //
  // `setBackgroundInert` above is the primary defence against Tab
  // escape into obscured page content, but `inert` is Baseline 2023 —
  // older browsers silently ignore `el.inert = true` and would let Tab
  // wander off the last sidebar link into the still-focusable page
  // behind. Wrap focus inside the sidebar with a keydown listener as a
  // belt-and-suspenders fallback: on a Tab outside the sidebar (e.g.
  // user moved focus there before `inert` was applied), bring it back;
  // on a forward Tab from the last focusable, wrap to the first; on a
  // back-Tab from the first, wrap to the last. On browsers that DO
  // support `inert` this is mostly a no-op (Tab from the last
  // focusable just lands on the same element again because nothing
  // else is reachable), so the cost of always-on is negligible.
  document.addEventListener("keydown", function (event) {
    if (event.key !== "Tab") return;
    if (toggle.getAttribute("aria-expanded") !== "true") return;
    var focusable = sidebar.querySelectorAll(
      "a[href], button:not([disabled])"
    );
    if (focusable.length === 0) return;
    var first = focusable[0];
    var last = focusable[focusable.length - 1];
    if (!sidebar.contains(document.activeElement)) {
      event.preventDefault();
      first.focus();
      return;
    }
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  });

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
    // Rule fragments are `#/rule/<name>`, whose `/` characters make
    // them invalid CSS id selectors — `querySelector("#/rule/...")`
    // would throw. Resolve the raw id with getElementById instead,
    // which matches the literal `id` attribute and accepts slashes.
    // decodeURIComponent throws on malformed percent-encoding; we let
    // that propagate rather than swallow it — an uncaught throw inside
    // this click handler only aborts the handler, and surfacing the
    // error aids debugging.
    var id = decodeURIComponent(hash.slice("#".length));
    var target = document.getElementById(id);
    if (!target) return;
    target.setAttribute("tabindex", "-1");
    target.focus({ preventScroll: true });
  });

  // ---- Force-close on cross-breakpoint resize ---------------------------
  //
  // If the user opens the drawer at a narrow viewport and then resizes
  // or rotates into the >=1100px band, the desktop CSS hides the close
  // button (`.nav-sidebar-close { display: none }`) while leaving the
  // body still scroll-locked and the background still `inert`. With
  // the close button gone and the page background dead, the only way
  // out would be a sidebar-link click — and even then the reader is
  // stranded if they didn't want to navigate. Watch the breakpoint
  // with `matchMedia` and tear the open state down on crossing.
  if (window.matchMedia) {
    var desktopMQ = window.matchMedia("(min-width: 1100px)");
    var handleBreakpoint = function () {
      if (desktopMQ.matches && toggle.getAttribute("aria-expanded") === "true") {
        closeSidebar();
      }
    };
    if (desktopMQ.addEventListener) {
      desktopMQ.addEventListener("change", handleBreakpoint);
    } else if (desktopMQ.addListener) {
      // Safari < 14 / older WebKit: legacy MediaQueryList API.
      desktopMQ.addListener(handleBreakpoint);
    }
  }

  // ---- Reveal the toggle -----------------------------------------------
  //
  // Everything above this line set up the behaviour the button needs to
  // actually work — IntersectionObserver fade, Visual Viewport API
  // sync, click handler, focus management, inert plumbing, breakpoint
  // recovery. Now that those are wired up, drop the `hidden` attribute
  // the Rust template emits, so the button appears in the page exactly
  // when it's functional. If any of the above threw before reaching
  // this line, the toggle stays hidden and the reader falls back to
  // the index table near the top of the page.
  toggle.hidden = false;
})();
