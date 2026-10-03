import "./App.css";
import "./i18n";

import React, { Suspense, useEffect } from "react";
import { HashRouter, Route, Routes, useLocation } from "react-router";
import { restoreMainWindowPage } from "@/services/platform/mainWindowGeometry";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ChartThemeProvider } from "./providers/ChartThemeProvider";
import { SettingsEffectsProvider } from "./providers/SettingsEffectsProvider";
import { UIHost } from "./ui/UIHost";

const PlotWindow = React.lazy(() =>
  import("@/modules/results/public").then((m) => ({ default: m.PlotWindow })),
);
const SourceInspectorWindow = React.lazy(() =>
  import("@/modules/results/public").then((m) => ({
    default: m.SourceInspectorWindow,
  })),
);
const LogWindow = React.lazy(() =>
  import("@/modules/logs/public").then((m) => ({ default: m.LogWindow })),
);
const WorkbenchComposition = React.lazy(() =>
  import("./windows/workbench/WorkbenchComposition").then((m) => ({
    default: m.WorkbenchComposition,
  })),
);
const ProjectPickerScreen = React.lazy(() =>
  import("@/modules/project-explorer/public").then((m) => ({
    default: m.ProjectPickerScreen,
  })),
);

function AppRouter() {
  const { pathname } = useLocation();
  const page = pathname === "/editor" ? "editor" : "projects";
  useEffect(() => {
    void restoreMainWindowPage(page);
  }, [page]);

  return (
    <Suspense fallback={null}>
      <Routes>
        <Route path="/" element={<ProjectPickerScreen />} />
        <Route path="/projects" element={<ProjectPickerScreen />} />
        <Route path="/editor" element={<WorkbenchComposition />} />
        <Route path="/plot" element={<PlotWindow />} />
        <Route path="/inspect" element={<SourceInspectorWindow />} />
        <Route path="/logs" element={<LogWindow />} />
        <Route path="*" element={<ProjectPickerScreen />} />
      </Routes>
    </Suspense>
  );
}

export default function App() {
  return (
    <TooltipProvider>
      <SettingsEffectsProvider>
        <ChartThemeProvider>
          <HashRouter>
            <AppRouter />
          </HashRouter>
          <UIHost />
        </ChartThemeProvider>
      </SettingsEffectsProvider>
    </TooltipProvider>
  );
}
