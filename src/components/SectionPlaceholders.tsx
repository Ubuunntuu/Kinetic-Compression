import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  FilePlus2,
  FolderOutput,
  FolderTree,
  Activity,
  Gauge,
  ShieldCheck,
  Search,
  Play,
} from "lucide-react";
import {
  ArchiveFormat,
  ArchiveInspection,
  CompressionMode,
  EngineStatusPayload,
  IntegrityReport,
  NavSection,
  OperationResult,
} from "../types/archive";

interface SectionWorkspaceProps {
  section: NavSection;
  status: EngineStatusPayload;
  onRefreshStatus: () => void;
}

export const SectionWorkspace: React.FC<SectionWorkspaceProps> = ({
  section,
  status,
  onRefreshStatus,
}) => {
  const [sourceInput, setSourceInput] = useState<string>(
    "C:\\projects\\kinetic compression\\docs"
  );
  const [outputPath, setOutputPath] = useState<string>(
    "C:\\projects\\kinetic compression\\bench_output\\sample.zip"
  );
  const [selectedFormat, setSelectedFormat] = useState<ArchiveFormat>(
    status.default_format
  );
  const [selectedMode, setSelectedMode] = useState<CompressionMode>(
    status.default_mode
  );
  const [opResult, setOpResult] = useState<OperationResult | null>(null);
  const [opError, setOpError] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);

  const [extractArchivePath, setExtractArchivePath] = useState<string>(
    "C:\\projects\\kinetic compression\\bench_output\\sample.zip"
  );
  const [extractDestPath, setExtractDestPath] = useState<string>(
    "C:\\projects\\kinetic compression\\bench_output\\extracted"
  );
  const [overwritePolicy, setOverwritePolicy] = useState<string>(
    status.config.overwrite_policy
  );

  const [explorerPath, setExplorerPath] = useState<string>(
    "C:\\projects\\kinetic compression\\bench_output\\sample.zip"
  );
  const [inspection, setInspection] = useState<ArchiveInspection | null>(null);
  const [validation, setValidation] = useState<IntegrityReport | null>(null);
  const [searchQuery, setSearchQuery] = useState<string>("");

  const handleCreateArchive = async () => {
    setBusy(true);
    setOpError(null);
    setOpResult(null);
    try {
      const sources = sourceInput
        .split(";")
        .map((s) => s.trim())
        .filter((s) => s.length > 0);
      const res = await invoke<OperationResult>("create_archive_cmd", {
        request: {
          source_paths: sources,
          output_path: outputPath,
          format: selectedFormat,
          mode: selectedMode,
        },
      });
      setOpResult(res);
      onRefreshStatus();
    } catch (err) {
      setOpError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleExtractArchive = async () => {
    setBusy(true);
    setOpError(null);
    setOpResult(null);
    try {
      const res = await invoke<OperationResult>("extract_archive_cmd", {
        request: {
          archive_path: extractArchivePath,
          destination_path: extractDestPath,
          overwrite_policy: overwritePolicy,
        },
      });
      setOpResult(res);
      onRefreshStatus();
    } catch (err) {
      setOpError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleInspectArchive = async () => {
    setBusy(true);
    setOpError(null);
    setValidation(null);
    try {
      const res = await invoke<ArchiveInspection>("inspect_archive_cmd", {
        archivePath: explorerPath,
      });
      setInspection(res);
      onRefreshStatus();
    } catch (err) {
      setOpError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleValidateArchive = async () => {
    setBusy(true);
    setOpError(null);
    try {
      const report = await invoke<IntegrityReport>("validate_archive_cmd", {
        archivePath: explorerPath,
      });
      setValidation(report);
    } catch (err) {
      setOpError(String(err));
    } finally {
      setBusy(false);
    }
  };

  if (section === "create") {
    return (
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <FilePlus2 size={16} /> Create Archive (.zip, .7z, .kinetic)
          </span>
          <span className="badge badge-accent">
            Atomic .part Staging Enabled
          </span>
        </div>

        <div className="form-group">
          <label className="form-label">
            Source Files or Folders (Separate multiple paths with semicolon ;)
          </label>
          <input
            type="text"
            className="form-input"
            value={sourceInput}
            onChange={(e) => setSourceInput(e.target.value)}
          />
        </div>

        <div className="form-group">
          <label className="form-label">Output Archive Path</label>
          <input
            type="text"
            className="form-input"
            value={outputPath}
            onChange={(e) => setOutputPath(e.target.value)}
          />
        </div>

        <div className="grid-2">
          <div className="form-group">
            <label className="form-label">Archive Format</label>
            <select
              className="form-select"
              value={selectedFormat}
              onChange={(e) => {
                const fmt = e.target.value as ArchiveFormat;
                setSelectedFormat(fmt);
                const ext =
                  status.supported_formats.find((f) => f.format === fmt)
                    ?.extension || ".zip";
                setOutputPath((prev) =>
                  prev.replace(/\.(zip|7z|kinetic)$/i, ext)
                );
              }}
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
            <label className="form-label">Compression Mode</label>
            <select
              className="form-select"
              value={selectedMode}
              onChange={(e) =>
                setSelectedMode(e.target.value as CompressionMode)
              }
            >
              {status.compression_modes.map((m) => (
                <option key={m.mode} value={m.mode}>
                  {m.mode} — {m.description}
                </option>
              ))}
            </select>
          </div>
        </div>

        <button
          className="btn btn-primary"
          onClick={handleCreateArchive}
          disabled={busy}
        >
          <Play size={15} /> {busy ? "Compressing..." : "Create Archive Now"}
        </button>

        {opError && (
          <div
            className="card"
            style={{ marginTop: "14px", borderColor: "var(--status-danger)" }}
          >
            <strong>Compression Error:</strong> {opError}
          </div>
        )}

        {opResult && (
          <div
            className="card"
            style={{ marginTop: "14px", borderColor: "var(--status-success)" }}
          >
            <div className="metric-row">
              <span>Status</span>
              <strong>{opResult.message}</strong>
            </div>
            <div className="metric-row">
              <span>Measured Throughput</span>
              <strong>{opResult.throughput_mb_s.toFixed(2)} MB/s</strong>
            </div>
            <div className="metric-row">
              <span>Compressed Ratio</span>
              <strong>{opResult.compression_ratio_percent.toFixed(1)}%</strong>
            </div>
          </div>
        )}
      </div>
    );
  }

  if (section === "extract") {
    return (
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <FolderOutput size={16} /> Extract Archive (.zip, .7z, .rar, .kinetic)
          </span>
          <span className="badge badge-success">
            <ShieldCheck size={13} /> Zip-Slip &amp; Bomb Guard Active
          </span>
        </div>

        <div className="form-group">
          <label className="form-label">Archive File Path</label>
          <input
            type="text"
            className="form-input"
            value={extractArchivePath}
            onChange={(e) => setExtractArchivePath(e.target.value)}
          />
        </div>

        <div className="grid-2">
          <div className="form-group">
            <label className="form-label">Destination Folder</label>
            <input
              type="text"
              className="form-input"
              value={extractDestPath}
              onChange={(e) => setExtractDestPath(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">Overwrite Policy</label>
            <select
              className="form-select"
              value={overwritePolicy}
              onChange={(e) => setOverwritePolicy(e.target.value)}
            >
              <option value="overwrite">Overwrite Existing Files</option>
              <option value="skip">Skip Existing Files</option>
              <option value="rename">Auto-Rename Extracted Files</option>
              <option value="ask">Block &amp; Alert If File Exists</option>
            </select>
          </div>
        </div>

        <button
          className="btn btn-primary"
          onClick={handleExtractArchive}
          disabled={busy}
        >
          <FolderOutput size={15} />{" "}
          {busy ? "Extracting..." : "Extract Archive Safely"}
        </button>

        {opError && (
          <div
            className="card"
            style={{ marginTop: "14px", borderColor: "var(--status-danger)" }}
          >
            <strong>Extraction Blocked / Error:</strong> {opError}
          </div>
        )}

        {opResult && (
          <div
            className="card"
            style={{ marginTop: "14px", borderColor: "var(--status-success)" }}
          >
            <div className="metric-row">
              <span>Status</span>
              <strong>{opResult.message}</strong>
            </div>
            <div className="metric-row">
              <span>Extraction Speed</span>
              <strong>{opResult.throughput_mb_s.toFixed(2)} MB/s</strong>
            </div>
          </div>
        )}
      </div>
    );
  }

  if (section === "explorer") {
    const filteredEntries =
      inspection?.entries.filter((e) =>
        e.path.toLowerCase().includes(searchQuery.toLowerCase())
      ) || [];

    return (
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <FolderTree size={16} /> Archive Explorer &amp; Integrity Validator
          </span>
          {inspection && (
            <span className="badge badge-accent">
              Format: {inspection.format} • {inspection.total_entries} Entries
            </span>
          )}
        </div>

        <div
          style={{
            display: "flex",
            gap: "10px",
            marginBottom: "14px",
            alignItems: "center",
          }}
        >
          <input
            type="text"
            className="form-input"
            style={{ flex: 1 }}
            placeholder="Enter path to .zip, .7z, .rar, or .kinetic archive..."
            value={explorerPath}
            onChange={(e) => setExplorerPath(e.target.value)}
          />
          <button
            className="btn btn-primary"
            onClick={handleInspectArchive}
            disabled={busy}
          >
            <Search size={15} /> Browse Archive
          </button>
          <button
            className="btn"
            onClick={handleValidateArchive}
            disabled={busy}
          >
            <ShieldCheck size={15} /> Verify Checksums
          </button>
        </div>

        {opError && (
          <div
            className="card"
            style={{
              marginBottom: "14px",
              borderColor: "var(--status-danger)",
            }}
          >
            <strong>Archive Explorer Notice:</strong> {opError}
          </div>
        )}

        {validation && (
          <div
            className="card"
            style={{
              marginBottom: "14px",
              borderColor: "var(--status-success)",
            }}
          >
            <strong>Integrity Verification Passed:</strong> {validation.message}
          </div>
        )}

        {inspection && (
          <>
            <div style={{ marginBottom: "12px" }}>
              <input
                type="text"
                className="form-input"
                style={{ width: "100%" }}
                placeholder="Filter entries by file or folder name..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>

            <table className="data-table">
              <thead>
                <tr>
                  <th>Path</th>
                  <th>Type</th>
                  <th>Original Size</th>
                  <th>Compressed</th>
                  <th>Codec</th>
                  <th>CRC / Digest</th>
                </tr>
              </thead>
              <tbody>
                {filteredEntries.map((entry) => (
                  <tr key={entry.path}>
                    <td>{entry.path}</td>
                    <td>{entry.is_dir ? "Folder" : "File"}</td>
                    <td>{(entry.uncompressed_size / 1024).toFixed(1)} KB</td>
                    <td>{(entry.compressed_size / 1024).toFixed(1)} KB</td>
                    <td>{entry.codec_name}</td>
                    <td>{entry.checksum_hex || "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </div>
    );
  }

  if (section === "performance") {
    return (
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <Activity size={16} /> Real-Time Performance Monitor
          </span>
          <span className="badge">
            {status.hardware.logical_cores} Cores •{" "}
            {status.hardware.available_memory_mb} MB Free RAM
          </span>
        </div>
        <div className="empty-state">
          Live atomic worker thread telemetry, ring-buffer memory arena usage,
          and pause/resume/cancel controls connect in <strong>Phase 4</strong>.
        </div>
      </div>
    );
  }

  return (
    <div className="card">
      <div className="card-header">
        <span className="card-title">
          <Gauge size={16} /> Reproducible Benchmark Center (vs. 7-Zip &amp;
          WinRAR)
        </span>
        <span className="badge badge-accent">No Fabricated Metrics</span>
      </div>
      <div className="empty-state">
        Automated comparative benchmark harness against local 7-Zip and WinRAR
        binaries connects in <strong>Phase 9</strong>.
      </div>
    </div>
  );
};