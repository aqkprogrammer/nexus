import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { bootstrap, toast } from "./stores";
import "./styles/index.css";

bootstrap().catch((error) => toast.error("NEXUS could not finish starting", error));

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
