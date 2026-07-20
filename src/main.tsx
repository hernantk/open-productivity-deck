import "@fontsource-variable/bricolage-grotesque";
import "@fontsource-variable/manrope";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

const storedTheme = localStorage.getItem("opd-theme");
const theme = storedTheme === "light" || storedTheme === "dark"
  ? storedTheme
  : window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
document.documentElement.dataset.theme = theme;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
