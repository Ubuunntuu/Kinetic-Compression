import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  EngineStatusPayload,
  NavSection,
  UserConfig,
} from "./types/archive";
import { Sidebar } from "./components/Sidebar";
import { Dashboard } from "./components/Dashboard";
import { SettingsView } from "./components/SettingsView";
import { AboutView } from "./components/AboutView";
import { SectionWorkspace } from "./components/SectionPlaceholders";

const SECTION_TITLES: Record<NavSection, string> = {
  dashboard: "Home / Dashboard",
  create: "Create Archive",
  extract: "Extract Archive",
  explorer: "Archive Explorer",
  "compression-settings": "Compression Settings",
  performance: "Performance Monitor",
  benchmark: "Benchmark Center",
  settings: "Application Settings",
  about: "About Kinetic",
};

export const App: React.FC = () => {
  const [status, setStatus] = useState<EngineStatusPayload | null>(null);
  const [activeSection, setActiveSection] = useState<NavSection>("dashboard");
  const [theme, setTheme] = useState<"dark" | "light">("dark");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<EngineStatusPayload>("get_engine_status")
      .then((payload) => {
        setStatus(payload);
        const initialTheme = payload.config.theme === "light" ? "light" : "dark";
        setTheme(initialTheme);
        document.documentElement.setAttribute("data-theme", initialTheme);
      })
      .catch((err) => setError(String(err)));
  }, []);

  const handleSaveConfig = async (newConfig: UserConfig) => {
    try {
      const updated = await invoke<EngineStatusPayload>("update_user_config", {
        newConfig,
      });
      setStatus(updated);
      const nextTheme = updated.config.theme === "light" ? "light" : "dark";
      setTheme(nextTheme);
      document.documentElement.setAttribute("data-theme", nextTheme);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleToggleTheme = async () => {
    const nextTheme = theme === "dark" ? "light" : "dark";
    setTheme(nextTheme);
    document.documentElement.setAttribute("data-theme", nextTheme);
    if (status) {
      await handleSaveConfig({ ...status.config, theme: nextTheme });
    }
  };

  const handleClearHistory = async () => {
    try {
      const updated = await invoke<EngineStatusPayload>("clear_recent_history");
      setStatus(updated);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="app-layout">
      <Sidebar
        activeSection={activeSection}
        onSelectSection={setActiveSection}
        theme={theme}
        onToggleTheme={handleToggleTheme}
        engineVersion={status ? status.engine_version : "0.2.0"}
      />

      <main className="main-workspace">
        <header className="topbar">
          <div className="topbar-title">{SECTION_TITLES[activeSection]}</div>
          <div className="topbar-meta">
            {status && (
              <>
                <span className="badge">
                  Default: <strong>{status.default_format}</strong> (
                  {status.supported_formats.find((f) => f.format === status.default_format)?.extension})
                </span>
                <span className="badge badge-accent">
                  Mode: <strong>{status.default_mode}</strong>
                </span>
                <span className="badge badge-success">
                  {status.config.worker_threads} Threads Active
                </span>
              </>
            )}
          </div>
        </header>

        <div className="workspace-scroll">
          {error && (
            <div className="card" style={{ borderColor: "var(--status-danger)" }}>
              <strong>Engine IPC Notice:</strong> {error}
            </div>
          )}

          {!status ? (
            <div className="card">Initializing Kinetic Engine telemetry...</div>
          ) : activeSection === "dashboard" ? (
            <Dashboard
              status={status}
              onNavigate={setActiveSection}
              onClearHistory={handleClearHistory}
            />
          ) : activeSection === "compression-settings" || activeSection === "settings" ? (
            <SettingsView
              status={status}
              sectionType={activeSection}
              onSaveConfig={handleSaveConfig}
            />
          ) : activeSection === "about" ? (
            <AboutView status={status} />
          ) : (
            <SectionWorkspace section={activeSection} status={status} />
          )}
        </div>
      </main>
    </div>
  );
};

export default App;