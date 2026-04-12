import { useEffect } from "react";
import { HashRouter, Route, Routes, Navigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { setupEventBridge } from "./api/commands";
import AppShell from "./components/layout/AppShell";
import ScanPage from "./pages/ScanPage";
import GalleryPage from "./pages/GalleryPage";
import DuplicatesPage from "./pages/DuplicatesPage";
import JobsPage from "./pages/JobsPage";
import SettingsPage from "./pages/SettingsPage";

export default function App() {
  const queryClient = useQueryClient();

  useEffect(() => {
    setupEventBridge(queryClient);
  }, [queryClient]);

  return (
    <HashRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<Navigate to="/scan" replace />} />
          <Route path="/scan" element={<ScanPage />} />
          <Route path="/gallery" element={<GalleryPage />} />
          <Route path="/duplicates" element={<DuplicatesPage />} />
          <Route path="/jobs" element={<JobsPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
