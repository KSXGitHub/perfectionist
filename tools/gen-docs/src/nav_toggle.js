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

  // ---- Visual Viewport API ----------------------------------------------
  //
  // Some mobile browsers (Galaxy M15 / Android Chrome and Firefox among
  // them) anchor `position: fixed` to the *layout* viewport, not the
  // *visual* viewport — so when the URL bar collapses on scroll-down,
  // the supposedly-fixed hamburger ends up above the visible area and
  // appears to "scroll away" with the page. Translate the toggle by the
  // visual viewport's offset to compensate.
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
  // The drawer is opened by tapping `.nav-toggle` and closed by tapping
  // `.nav-sidebar-close` (the ✕ inside the overlay). On narrow viewports
  // we lock body scroll while it's open, which:
  //   * stops the URL bar from collapsing/expanding under the overlay
  //     (otherwise the overlay's fixed positioning drifts on browsers
  //     with the visual-viewport quirk above), and
  //   * stops the page behind from scrolling when the user swipes
  //     within the overlay.
  // We preserve the scroll position by snapping body to `top: -<y>px`
  // while locked and restoring `scrollTo(0, y)` on unlock.
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
