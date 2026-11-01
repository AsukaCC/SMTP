"use strict";
(function () {
  const key = "bunmail.dashboard.theme";
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  function choice() {
    try {
      const value = localStorage.getItem(key);
      if (value === "light" || value === "dark" || value === "system") return value;
    } catch (_) {}
    return "system";
  }
  function resolved(value) {
    if (value === "light" || value === "dark") return value;
    return media.matches ? "dark" : "light";
  }
  function apply(value) {
    const theme = resolved(value);
    const root = document.documentElement;
    root.dataset.theme = theme;
    root.dataset.themeChoice = value;
    root.style.colorScheme = theme;
    const meta = document.querySelector('meta[name="color-scheme"]');
    if (meta) meta.setAttribute("content", theme);
  }
  function set(value) {
    if (value !== "light" && value !== "dark" && value !== "system") value = "system";
    try { localStorage.setItem(key, value); } catch (_) {}
    apply(value);
  }
  apply(choice());
  media.addEventListener("change", () => { if (choice() === "system") apply("system"); });
  window.bunmailTheme = { choice, set };
})();
