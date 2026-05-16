(function () {
  var table = document.querySelector("table.index");
  var toggle = document.querySelector(".nav-toggle");
  if (table && toggle) {
    if ("IntersectionObserver" in window) {
      var observer = new IntersectionObserver(function (entries) {
        var entry = entries[entries.length - 1];
        toggle.classList.toggle("nav-toggle-visible", !entry.isIntersecting);
      });
      observer.observe(table);
    } else {
      toggle.classList.add("nav-toggle-visible");
    }
  }

  // Close the drawer when the user follows a link inside the sidebar.
  // On narrow viewports the <nav> sits on top of the page; without
  // this, navigating to a rule anchor would leave the open sidebar
  // covering the target. On wide viewports the [open] attribute has
  // no visual effect, so removing it is harmless.
  var details = document.querySelector("details.nav-drawer");
  var sidebar = document.querySelector(".nav-sidebar");
  if (details && sidebar) {
    sidebar.addEventListener("click", function (event) {
      if (event.target.closest("a")) {
        details.removeAttribute("open");
      }
    });
  }
})();
