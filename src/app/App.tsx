import "@/utils/appLogger";
import "./App.css";

import React, { Suspense } from "react";
import { HashRouter, Route, Routes } from "react-router";
import { SettingsEffectsProvider } from "./providers/SettingsEffectsProvider";
import { UIHost } from "./ui/UIHost";

const PlotWindow = React.lazy(() => import("@/views/PlotView/PlotWindow").then(m => ({ default: m.PlotWindow })));
const DataViewWindow = React.lazy(() => import("@/views/DataView/DataViewWindow").then(m => ({ default: m.DataViewWindow })));
const LogWindow = React.lazy(() => import("@/views/LogView/LogWindow").then(m => ({ default: m.LogWindow })));
const InfoWindow = React.lazy(() => import("@/views/InfoView/InfoWindow").then(m => ({ default: m.InfoWindow })));
const EditorWindow = React.lazy(() => import("@/views/EditorView/EditorWindow").then(m => ({ default: m.EditorWindow })));

function AppRouter() {
  return (
    <Suspense fallback={null}>
      <Routes>
        <Route path="/plot" element={<PlotWindow />} />
        <Route path="/dataview" element={<DataViewWindow />} />
        <Route path="/logs" element={<LogWindow />} />
        <Route path="/info" element={<InfoWindow />} />
        <Route path="*" element={<EditorWindow />} />
      </Routes>
    </Suspense>
  );
}

export default function App() {
  return (
    <SettingsEffectsProvider>
      <HashRouter>
        <AppRouter />
      </HashRouter>
      <UIHost />
    </SettingsEffectsProvider>
  );
}
