import "./App.css";
import { setupLogger } from "@/shared/utils";

setupLogger();

import { PlotWindow } from "@/views/PlotView/PlotWindow";
import { DataWindow } from "@/views/DataView/DataWindow";
import { LogWindow } from "@/views/LogView/LogWindow";
import { EditorWindow } from "@/views/EditorView/EditorWindow";
import { SettingsProvider } from "./providers/SettingsProvider";


const isPlotWindow = window.location.hash === "#/plot";
const isDataWindow = window.location.hash === "#/data";
const isLogsWindow = window.location.hash === "#/logs";

function AppRouter() {
  if (isPlotWindow) return <PlotWindow />;
  if (isDataWindow) return <DataWindow />;
  if (isLogsWindow) return <LogWindow />;

  return <EditorWindow />;
}

export default function App() {
  return (
    <SettingsProvider>
      <AppRouter />
    </SettingsProvider>
  );
}
