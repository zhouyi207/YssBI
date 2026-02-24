import "@/utils/appLogger";
import "./App.css";

import { PlotWindow } from "@/views/PlotView/PlotWindow";
import { DataViewWindow } from "@/views/DataView/DataViewWindow";
import { LogWindow } from "@/views/LogView/LogWindow";
import { InfoWindow } from "@/views/InfoView/InfoWindow";
import { EditorWindow } from "@/views/EditorView/EditorWindow";
import { SettingsEffectsProvider } from "./providers/SettingsEffectsProvider";

const hash = window.location.hash;
const isPlotWindow = hash === "#/plot";
const isDataViewWindow = hash === "#/dataview";
const isLogsWindow = hash === "#/logs";
const isInfoWindow = hash.startsWith("#/info");

function AppRouter() {
  if (isPlotWindow) return <PlotWindow />;
  if (isDataViewWindow) return <DataViewWindow />;
  if (isLogsWindow) return <LogWindow />;
  if (isInfoWindow) return <InfoWindow />;

  return <EditorWindow />;
}

export default function App() {
  return (
    <SettingsEffectsProvider>
      <AppRouter />
    </SettingsEffectsProvider>
  );
}
