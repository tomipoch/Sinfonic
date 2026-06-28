import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { ThemeProvider } from "@/modules/theme/ThemeProvider";
import { SettingsApp } from "./SettingsApp";
import "@/index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <ThemeProvider>
      <SettingsApp />
    </ThemeProvider>
  </StrictMode>,
);
