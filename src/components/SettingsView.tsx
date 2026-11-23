import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Sliders, Save, Shield, Cpu, FolderPlus, CheckCircle2 } from "lucide-react";
import {
  ArchiveFormat,
  CompressionMode,
  EngineStatusPayload,
  UserConfig,
} from "../types/archive";

interface ShellIntegrationStatus {
  registered: boolean;
  executable_path: string;
  registered_verbs: string[];
  message: string;
}

interface SettingsViewProps {
  status: EngineStatusPayload;
  sectionType: "compression-settings" | "settings";
  onSaveConfig: (newConfig: UserConfig) => Promise<void>;
}

export const SettingsView: React.FC<SettingsViewProps> = ({
  status,
  sectionType,
  onSaveConfig,
}) => {
  const [draft, setDraft] = useState<UserConfig>(status.config);
  const [savedNotice, setSavedNotice] = useState<boolean>(false);
  const [shellStatus, setShellStatus] = useState<ShellIntegrationStatus | null>(
    null
  );
  const [shellBusy, setShellBusy] = useState<boolean>(false);

  useEffect(() => {
    invoke<ShellIntegrationStatus>("get_shell_integration_status_cmd")
      .then(setShellStatus)
      .catch(() => {});
  }, []);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    await onSaveConfig(draft);
    setSavedNotice(true);
    setTimeout(() => setSavedNotice(false), 2500);
  };

  const handleRegisterShell = async () => {
    setShellBusy(true);
    try {
      const res = await invoke<ShellIntegrationStatus>(
        "register_shell_context_menu_cmd"
      );
      setShellStatus(res);
    } finally {
      setShellBusy(false);
    }
  };

  const handleUnregisterShell = async () => {
    setShellBusy(true);
    try {
      const res = await invoke<ShellIntegrationStatus>(
        "unregister_shell_context_menu_cmd"
      );
      setShellStatus(res);
    } finally {
      setShellBusy(false);
    }
  };

  return (
    <form
      onSubmit={handleSave}
      style={{ display: "flex", flexDirection: "column", gap: "18px" }}
    >
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <Sliders size={16} />
            {sectionType === "compression-settings"
              ? "Kinetic Engine Compression Pipeline Settings"
              : "General Application & Format Preferences"}
          </span>
          {savedNotice && (
            <span className="badge badge-success">Configuration Saved</span>
          )}
        </div>

        <div className="grid-2">
          <div className="form-group">
            <label className="form-label">
              Default Archive Creation Format (.zip Default)
            </label>
            <select
              className="form-select"
              value={draft.default_format}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  default_format: e.target.value as ArchiveFormat,
                })
              }
            >
              {status.supported_formats
                .filter((f) => f.can_create)
                .map((f) => (
                  <option key={f.format} value={f.format}>
                    {f.format} ({f.extension})
                  </option>
                ))}
            </select>
          </div>

          <div className="form-group">
            <label className="form-label">Default Compression Mode</label>
            <select
              className="form-select"
              value={draft.default_mode}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  default_mode: e.target.value as CompressionMode,
                })
              }
            >
              {status.compression_modes.map((m) => (
                <option key={m.mode} value={m.mode}>
                  {m.mode} — {m.description}
                </option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <label className="form-label">
              <Cpu size={14} /> Worker Threads (Max Logical Cores:{" "}
              {status.hardware.logical_cores})
            </label>
            <input
              type="number"
              className="form-input"
              min={1}
              max={status.hardware.logical_cores}
              value={draft.worker_threads}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  worker_threads: Number(e.target.value) || 1,
                })
              }
            />
          </div>

          <div className="form-group">
            <label className="form-label">
              Bounded Memory Arena Cap (MB) (Max Safe:{" "}
              {status.hardware.max_safe_memory_arena_mb} MB)
            </label>
            <input
              type="number"
              className="form-input"
              min={128}
              max={status.hardware.max_safe_memory_arena_mb}
              step={128}
              value={draft.memory_arena_mb}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  memory_arena_mb: Number(e.target.value) || 512,
                })
              }
            />
          </div>
        </div>
      </div>

      {/* Windows File Explorer Right-Click Context Menu Integration Card */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <FolderPlus size={16} /> Windows File Explorer Right-Click Context
            Menu Integration
          </span>
          <span
            className={`badge ${
              shellStatus?.registered ? "badge-success" : "badge-accent"
            }`}
          >
            {shellStatus?.registered
              ? "Active in Windows Explorer"
              : "Not Registered"}
          </span>
        </div>

        <p
          style={{
            fontSize: "13px",
            color: "var(--text-secondary)",
            marginBottom: "12px",
          }}
        >
          Adds <strong>Kinetic: Compress to .zip</strong>,{" "}
          <strong>Kinetic: Compress to .kinetic</strong>,{" "}
          <strong>Kinetic: Extract Here</strong>, and{" "}
          <strong>Kinetic: Extract to Subfolder</strong> directly to your
          Windows right-click menu under <code>HKCU\Software\Classes</code> (no
          administrator UAC prompt required).
        </p>

        {shellStatus && (
          <div style={{ marginBottom: "12px", fontSize: "12.5px" }}>
            <div className="metric-row">
              <span>Status</span>
              <strong>{shellStatus.message}</strong>
            </div>
            {shellStatus.registered_verbs.length > 0 && (
              <div className="metric-row">
                <span>Registered Explorer Verbs</span>
                <strong style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                  <CheckCircle2 size={14} color="var(--status-success)" />
                  {shellStatus.registered_verbs.join(" • ")}
                </strong>
              </div>
            )}
          </div>
        )}

        <div style={{ display: "flex", gap: "10px" }}>
          <button
            type="button"
            className="btn btn-primary"
            onClick={handleRegisterShell}
            disabled={shellBusy}
          >
            <FolderPlus size={14} /> Enable Windows Explorer Context Menus
          </button>
          <button
            type="button"
            className="btn"
            onClick={handleUnregisterShell}
            disabled={shellBusy}
          >
            Remove Explorer Context Menus
          </button>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <Shield size={16} /> Kinetic Engine Advanced Capabilities &amp;
            Safety Policies
          </span>
        </div>

        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "12px",
            fontSize: "13.5px",
          }}
        >
          <label style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <input
              type="checkbox"
              checked={draft.enable_fastcdc_dedup}
              onChange={(e) =>
                setDraft({ ...draft, enable_fastcdc_dedup: e.target.checked })
              }
            />
            <span>
              <strong>FastCDC Content-Defined Deduplication</strong> — Eliminate
              duplicate chunks across the archive using rolling Gear hashes
            </span>
          </label>

          <label style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <input
              type="checkbox"
              checked={draft.enable_entropy_bypass}
              onChange={(e) =>
                setDraft({ ...draft, enable_entropy_bypass: e.target.checked })
              }
            />
            <span>
              <strong>Shannon Entropy Media Bypass (H &gt; 7.85)</strong> —
              Automatically store incompressible media streams at memory speed
            </span>
          </label>

          <label style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <input
              type="checkbox"
              checked={draft.enable_kinetic_shield_parity}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  enable_kinetic_shield_parity: e.target.checked,
                })
              }
            />
            <span>
              <strong>Kinetic Shield Parity Blocks</strong> — Append recovery
              parity blocks for automatic self-healing against disk corruption
            </span>
          </label>

          <label style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <input
              type="checkbox"
              checked={draft.verify_integrity_on_complete}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  verify_integrity_on_complete: e.target.checked,
                })
              }
            />
            <span>
              <strong>Post-Operation BLAKE3 &amp; XXH3-64 Verification</strong>{" "}
              — Cryptographically verify archive blocks immediately after
              creation
            </span>
          </label>
        </div>

        <div style={{ marginTop: "18px" }}>
          <button type="submit" className="btn btn-primary">
            <Save size={15} /> Save Configuration
          </button>
        </div>
      </div>
    </form>
  );
};