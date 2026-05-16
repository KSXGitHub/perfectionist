(function () {
  // Hide the hamburger while the index table is on screen. Default
  // is visible — if this branch doesn't run (no IntersectionObserver,
  // CSP-blocked script, etc.) the drawer remains reachable.
  var table = document.querySelector("table.index");
  var toggle = document.querySelector(".nav-toggle");
  if (table && toggle && "IntersectionObserver" in window) {
    var observer = new IntersectionObserver(function (entries) {
      var entry = entries[entries.length - 1];
      toggle.classList.toggle("nav-toggle-hidden", entry.isIntersecting);
    });
    observer.observe(table);
  }

  // Close the drawer when the user follows a link inside the sidebar.
  // On narrow viewports the <nav> sits on top of the page; without
  // this, navigating to a rule anchor would leave the open sidebar
  // covering the target. On wide viewports the [open] attribute has
  // no visual effect, so removing it is harmless.
  //
  // After closing, move focus to the destination so keyboard users
  // don't lose their place: the just-clicked <a> is now inside a
  // display:none sidebar and focus would otherwise drop to <body>.
  // Rule articles and the catalogue heading aren't focusable by
  // default, so set tabindex="-1" before .focus() — that makes them
  // programmatically focusable without putting them in the tab order.
  var details = document.querySelector("details.nav-drawer");
  var sidebar = document.querySelector(".nav-sidebar");
  if (details && sidebar) {
    sidebar.addEventListener("click", function (event) {
      var link = event.target.closest("a");
      if (!link) return;
      // Skip anything that isn't an unmodified primary-button
      // activation: Ctrl/Cmd/Shift/Alt-clicks and middle-clicks are
      // standard gestures for "open in new tab/window", which leave
      // the current page intact — closing the drawer or moving
      // focus here would surprise the user.
      if (event.button !== 0) return;
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      details.removeAttribute("open");
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
  }
})();
