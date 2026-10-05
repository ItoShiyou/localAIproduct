import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { createApi } from "./api";
import "./styles.css";
import "./calm.css";

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App api={createApi()} />
  </React.StrictMode>,
);
