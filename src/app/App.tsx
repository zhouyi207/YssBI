import "@/utils/logger";
import "./App.css";

import { PlotWindow } from "@/views/PlotView/PlotWindow";
import { DataViewWindow } from "@/views/DataView/DataViewWindow";
import { LogWindow } from "@/views/LogView/LogWindow";
import { EditorWindow } from "@/views/EditorView/EditorWindow";
import { ThemeProvider } from "@/app/providers/ThemeContext";

const isPlotWindow = window.location.hash === "#/plot";
const isDataViewWindow = window.location.hash === "#/dataview";
const isLogsWindow = window.location.hash === "#/logs";

function AppRouter() {
  if (isPlotWindow) return <PlotWindow />;
  if (isDataViewWindow) return <DataViewWindow />;
  if (isLogsWindow) return <LogWindow />;

  return <EditorWindow />;
}

export default function App() {
  return (
    <ThemeProvider>
      <AppRouter />
    </ThemeProvider>
  );
}
