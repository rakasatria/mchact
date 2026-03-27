import type { Appearance, UiTheme } from "./types";
import { UI_THEME_OPTIONS } from "./constants";

export function readAppearance(): Appearance {
  const saved = localStorage.getItem("microclaw_appearance");
  return saved === "light" ? "light" : "dark";
}

export function saveAppearance(value: Appearance): void {
  localStorage.setItem("microclaw_appearance", value);
}

export function readUiTheme(): UiTheme {
  const saved = localStorage.getItem("microclaw_ui_theme") as UiTheme | null;
  return UI_THEME_OPTIONS.some((t) => t.key === saved)
    ? (saved as UiTheme)
    : "green";
}

export function saveUiTheme(value: UiTheme): void {
  localStorage.setItem("microclaw_ui_theme", value);
}

if (typeof document !== "undefined") {
  document.documentElement.classList.toggle(
    "dark",
    readAppearance() === "dark",
  );
  document.documentElement.setAttribute("data-ui-theme", readUiTheme());
}
