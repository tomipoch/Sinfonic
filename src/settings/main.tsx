import { StrictMode } from "react";
import ReactDOM from "react-dom/client";

import { SettingsApp } from "./SettingsApp";
import { ThemeProvider } from "@/modules/theme/ThemeProvider";
import "@/index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <ThemeProvider>
      <SettingsApp />
    </ThemeProvider>
  </StrictMode>,
);
