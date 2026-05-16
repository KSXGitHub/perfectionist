(function () {
  var table = document.querySelector("table.index");
  var toggle = document.querySelector(".nav-toggle");
  if (!table || !toggle) return;
  if (!("IntersectionObserver" in window)) {
    toggle.classList.add("nav-toggle-visible");
    return;
  }
  var observer = new IntersectionObserver(function (entries) {
    var entry = entries[entries.length - 1];
    toggle.classList.toggle("nav-toggle-visible", !entry.isIntersecting);
  });
  observer.observe(table);
})();
