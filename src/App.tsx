import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { EngineStatusPayload } from "./types/archive";

export const App: React.FC = () => {
  const [status, setStatus] = useState<EngineStatusPayload | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<EngineStatusPayload>("get_engine_status")
      .then((payload) => setStatus(payload))
      .catch((err) => setError(String(err)));
  }, []);

  return (
    <div className="shell-container">
      <header className="shell-header">
        <div>
          <h1 style={{ fontSize: "1.35rem", fontWeight: 600 }}>
            KINETIC — Advanced Compression & Archive Manager
          </h1>
          <p style={{ color: "var(--text-secondary)", fontSize: "0.85rem", marginTop: "4px" }}>
            Phase 1 Foundation Shell • Native Rust Engine + Tauri 2 IPC Bridge
          </p>
        </div>
        <span className="badge">
          {status ? `${status.engine_name} v${status.engine_version}` : "Connecting Engine..."}
        </span>
      </header>

      {error && (
        <div className="shell-card" style={{ borderColor: "#ef4444" }}>
          <h3>IPC Diagnostic Notice</h3>
          <p>{error}</p>
        </div>
      )}

      {status && (
        <div className="shell-grid">
          <div className="shell-card">
            <h3>Hardware Telemetry Profile</h3>
            <div className="metric-row">
              <span>Logical CPU Cores</span>
              <strong>{status.hardware.logical_cores} Cores</strong>
            </div>
            <div className="metric-row">
              <span>Worker Thread Pool</span>
              <strong>{status.hardware.recommended_threads} Threads</strong>
            </div>
            <div className="metric-row">
              <span>Total System Memory</span>
              <strong>{status.hardware.total_memory_mb} MB</strong>
            </div>
            <div className="metric-row">
              <span>Bounded Memory Arena Cap</span>
              <strong>{status.hardware.max_safe_memory_arena_mb} MB</strong>
            </div>
          </div>

          <div className="shell-card">
            <h3>Archive Format Matrix (Default: {status.default_format})</h3>
            {status.supported_formats.map((f) => (
              <div className="metric-row" key={f.format}>
                <span>
                  <strong>{f.format}</strong> ({f.extension})
                </span>
                <span>
                  {f.can_create ? "Create / Extract" : "Extract & Browse Only"}
                </span>
              </div>
            ))}
          </div>

          <div className="shell-card">
            <h3>Kinetic Engine Compression Modes</h3>
            {status.compression_modes.map((m) => (
              <div className="metric-row" key={m.mode}>
                <span>
                  <strong>{m.mode}</strong> ({Math.round(m.target_block_size_bytes / (1024 * 1024))} MB Block)
                </span>
                <span style={{ color: "var(--text-secondary)", fontSize: "0.8rem" }}>
                  {m.mode === status.default_mode ? "Default" : "Ready"}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

export default App;
