import React, { useState } from "react";
import { Sliders, Settings, Save, CheckCircle2 } from "lucide-react";
import {
  ArchiveFormat,
  CompressionMode,
  EngineStatusPayload,
  UserConfig,
} from "../types/archive";

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

  const handleApply = async () => {
    await onSaveConfig(draft);
    setSavedNotice(true);
    setTimeout(() => setSavedNotice(false), 2500);
  };

  if (sectionType === "compression-settings") {
    return (
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <Sliders size={16} /> Kinetic Engine Compression Settings
          </span>
          {savedNotice && (
            <span className="badge badge-success">
              <CheckCircle2 size={13} /> Saved to Rust Engine
            </span>
          )}
        </div>

        <div className="grid-2">
          <div className="form-group">
            <label className="form-label">Default Compression Mode</label>
            <span className="form-help">
              Determines block sizing, pre-filters, and codec routing
            </span>
            <select
              className="form-select"
              value={draft.default_mode}
              onChange={(e) =>
                setDraft({ ...draft, default_mode: e.target.value as CompressionMode })
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
              Worker Thread Pool ({draft.worker_threads} of {status.hardware.logical_cores} Logical Cores)
            </label>
            <span className="form-help">
              Number of parallel compression/extraction worker threads
            </span>
            <input
              type="number"
              className="form-input"
              min={1}
              max={status.hardware.logical_cores}
              value={draft.worker_threads}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  worker_threads: Math.max(
                    1,
                    Math.min(status.hardware.logical_cores, Number(e.target.value) || 1)
                  ),
                })
              }
            />
          </div>

          <div className="form-group">
            <label className="form-label">
              Bounded Memory Arena Limit ({draft.memory_arena_mb} MB)
            </label>
            <span className="form-help">
              Hard cap on pre-allocated ring buffers (Max safe: {status.hardware.max_safe_memory_arena_mb} MB)
            </span>
            <input
              type="number"
              className="form-input"
              min={256}
              max={status.hardware.max_safe_memory_arena_mb}
              step={256}
              value={draft.memory_arena_mb}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  memory_arena_mb: Math.max(256, Number(e.target.value) || 512),
                })
              }
            />
          </div>

          <div className="form-group">
            <label className="form-label">Advanced Kinetic Pipeline Features</label>
            <span className="form-help">
              Content-aware optimizations for the native .kinetic engine
            </span>
            <div style={{ display: "flex", flexDirection: "column", gap: "8px", marginTop: "6px" }}>
              <label style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <input
                  type="checkbox"
                  checked={draft.enable_fastcdc_dedup}
                  onChange={(e) =>
                    setDraft({ ...draft, enable_fastcdc_dedup: e.target.checked })
                  }
                />
                <span>Enable FastCDC Content-Defined Global Deduplication</span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <input
                  type="checkbox"
                  checked={draft.enable_entropy_bypass}
                  onChange={(e) =>
                    setDraft({ ...draft, enable_entropy_bypass: e.target.checked })
                  }
                />
                <span>Enable Shannon Entropy Media Bypass (H &gt; 7.85 bits/byte)</span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <input
                  type="checkbox"
                  checked={draft.enable_kinetic_shield_parity}
                  onChange={(e) =>
                    setDraft({ ...draft, enable_kinetic_shield_parity: e.target.checked })
                  }
                />
                <span>Include Kinetic Shield Parity Recovery Blocks (.kinetic format)</span>
              </label>
            </div>
          </div>
        </div>

        <button className="btn btn-primary" onClick={handleApply}>
          <Save size={15} /> Apply Compression Settings
        </button>
      </div>
    );
  }

  return (
    <div className="card">
      <div className="card-header">
        <span className="card-title">
          <Settings size={16} /> Application Preferences & Defaults
        </span>
        {savedNotice && (
          <span className="badge badge-success">
            <CheckCircle2 size={13} /> Preferences Saved
          </span>
        )}
      </div>

      <div className="grid-2">
        <div className="form-group">
          <label className="form-label">Default Archive Format (.zip by default)</label>
          <span className="form-help">
            Select default creation format (RAR creation disabled for license compliance)
          </span>
          <select
            className="form-select"
            value={draft.default_format}
            onChange={(e) =>
              setDraft({ ...draft, default_format: e.target.value as ArchiveFormat })
            }
          >
            {status.supported_formats
              .filter((f) => f.can_create)
              .map((f) => (
                <option key={f.format} value={f.format}>
                  {f.format} ({f.extension}) {f.format === "Zip" ? "— Default Standard" : f.format === "Kinetic" ? "— Native High-Performance" : ""}
                </option>
              ))}
          </select>
        </div>

        <div className="form-group">
          <label className="form-label">Application Theme</label>
          <span className="form-help">Windows 11 Dark mode by default with optional Light mode</span>
          <select
            className="form-select"
            value={draft.theme}
            onChange={(e) =>
              setDraft({ ...draft, theme: e.target.value as "dark" | "light" })
            }
          >
            <option value="dark">Dark Mode (Default)</option>
            <option value="light">Light Mode</option>
          </select>
        </div>

        <div className="form-group">
          <label className="form-label">Extraction Overwrite Policy</label>
          <span className="form-help">Safety behavior when target files already exist on disk</span>
          <select
            className="form-select"
            value={draft.overwrite_policy}
            onChange={(e) => setDraft({ ...draft, overwrite_policy: e.target.value })}
          >
            <option value="ask">Prompt Before Overwriting (Recommended)</option>
            <option value="skip">Skip Existing Files</option>
            <option value="rename">Auto-Rename Extracted Files</option>
          </select>
        </div>

        <div className="form-group">
          <label className="form-label">Post-Operation Integrity Verification</label>
          <span className="form-help">Automatically verify XXH3 / BLAKE3 / CRC32 checksums</span>
          <label style={{ display: "flex", alignItems: "center", gap: "8px", marginTop: "8px" }}>
            <input
              type="checkbox"
              checked={draft.verify_integrity_on_complete}
              onChange={(e) =>
                setDraft({ ...draft, verify_integrity_on_complete: e.target.checked })
              }
            />
            <span>Verify archive integrity immediately after creation</span>
          </label>
        </div>
      </div>

      <button className="btn btn-primary" onClick={handleApply}>
        <Save size={15} /> Save Application Settings
      </button>
    </div>
  );
};