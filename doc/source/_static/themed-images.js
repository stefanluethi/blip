// Shows the draw.io diagrams and the screenshots in the colours of the theme:
// the dark variant (name-dark.svg or name-dark.png, next to the light one) while
// the page is dark, also when the theme is switched. The links to the full-size
// images follow.
(function () {
  function variant(url, dark) {
    var light = url.replace(/-dark\.(svg|png)$/, ".$1");
    return dark ? light.replace(/\.(svg|png)$/, "-dark.$1") : light;
  }

  function apply() {
    var dark = document.documentElement.classList.contains("dark");
    document.querySelectorAll("img.diagram, img.screenshot").forEach(function (img) {
      var src = img.getAttribute("src");
      var wanted = variant(src, dark);
      if (wanted !== src) img.setAttribute("src", wanted);
      var link = img.closest("a");
      if (link) link.setAttribute("href", variant(link.getAttribute("href"), dark));
    });
  }

  document.addEventListener("DOMContentLoaded", function () {
    apply();
    new MutationObserver(apply).observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });
  });
})();
