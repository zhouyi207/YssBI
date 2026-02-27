import "@/utils/appLogger";
import "./App.css";

import React, { Suspense } from "react";
import { SettingsEffectsProvider } from "./providers/SettingsEffectsProvider";

const PlotWindow = React.lazy(() => import("@/views/PlotView/PlotWindow").then(m => ({ default: m.PlotWindow })));
const DataViewWindow = React.lazy(() => import("@/views/DataView/DataViewWindow").then(m => ({ default: m.DataViewWindow })));
const LogWindow = React.lazy(() => import("@/views/LogView/LogWindow").then(m => ({ default: m.LogWindow })));
const InfoWindow = React.lazy(() => import("@/views/InfoView/InfoWindow").then(m => ({ default: m.InfoWindow })));
const EditorWindow = React.lazy(() => import("@/views/EditorView/EditorWindow").then(m => ({ default: m.EditorWindow })));

const hash = window.location.hash;
const isPlotWindow = hash === "#/plot";
const isDataViewWindow = hash === "#/dataview";
const isLogsWindow = hash === "#/logs";
const isInfoWindow = hash.startsWith("#/info");

function AppRouter() {
  const content = (() => {
    if (isPlotWindow) return <PlotWindow />;
    if (isDataViewWindow) return <DataViewWindow />;
    if (isLogsWindow) return <LogWindow />;
    if (isInfoWindow) return <InfoWindow />;
    return <EditorWindow />;
  })();

  return <Suspense fallback={null}>{content}</Suspense>;
}

export default function App() {
  return (
    <SettingsEffectsProvider>
      <AppRouter />
    </SettingsEffectsProvider>
  );
}
