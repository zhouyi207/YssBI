import "@/utils/appLogger";
import "./App.css";
import "./i18n";

import React, { Suspense } from "react";
import { HashRouter, Route, Routes } from "react-router";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SettingsEffectsProvider } from "./providers/SettingsEffectsProvider";
import { UIHost } from "./ui/UIHost";

const PlotWindow = React.lazy(() => import("@/views/PlotView/PlotWindow").then(m => ({ default: m.PlotWindow })));
const DatabaseEditorWindow = React.lazy(() => import("@/views/DatabaseEditor/DatabaseEditorWindow").then(m => ({ default: m.DatabaseEditorWindow })));
const SourceInspectorWindow = React.lazy(() => import("@/views/SourceInspector/SourceInspectorWindow").then(m => ({ default: m.SourceInspectorWindow })));
const LogWindow = React.lazy(() => import("@/views/LogView/LogWindow").then(m => ({ default: m.LogWindow })));
const InfoWindow = React.lazy(() => import("@/views/InfoView/InfoWindow").then(m => ({ default: m.InfoWindow })));
const EditorWindow = React.lazy(() => import("@/views/EditorView/EditorWindow").then(m => ({ default: m.EditorWindow })));
const ProjectPickerScreen = React.lazy(() => import("@/views/ProjectView/ProjectPickerScreen").then(m => ({ default: m.ProjectPickerScreen })));

function AppRouter() {
  return (
    <Suspense fallback={null}>
      <Routes>
        <Route path="/" element={<ProjectPickerScreen />} />
        <Route path="/projects" element={<ProjectPickerScreen />} />
        <Route path="/editor" element={<EditorWindow />} />
        <Route path="/plot" element={<PlotWindow />} />
        <Route path="/database" element={<DatabaseEditorWindow />} />
        <Route path="/inspect" element={<SourceInspectorWindow />} />
        <Route path="/logs" element={<LogWindow />} />
        <Route path="/info" element={<InfoWindow />} />
        <Route path="*" element={<ProjectPickerScreen />} />
      </Routes>
    </Suspense>
  );
}

export default function App() {
  return (
    <TooltipProvider>
      <SettingsEffectsProvider>
        <HashRouter>
          <AppRouter />
        </HashRouter>
        <UIHost />
      </SettingsEffectsProvider>
    </TooltipProvider>
  );
}
