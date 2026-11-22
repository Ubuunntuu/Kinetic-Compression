import React, { useEffect, useState } from "react";
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
  Pause,
  RotateCcw,
  Square,
  Cpu,
} from "lucide-react";
import {
  ArchiveFormat,
  ArchiveInspection,
  CompressionMode,
  EngineStatusPayload,
  IntegrityReport,
  NavSection,
  OperationResult,
  PipelineJobReport,
  ProgressSnapshot,
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

  const [telemetry, setTelemetry] = useState<ProgressSnapshot | null>(null);
  const [pipelineMode, setPipelineMode] = useState<CompressionMode>(
    status.default_mode
  );
  const [payloadMb, setPayloadMb] = useState<number>(32);
  const [pipelineReport, setPipelineReport] =
    useState<PipelineJobReport | null>(null);

  useEffect(() => {
    if (section !== "performance") return;
    let active = true;
    const poll = () => {
      invoke<ProgressSnapshot>("get_live_telemetry_cmd")
        .then((snap) => {
          if (active) setTelemetry(snap);
        })
        .catch(() => {});
    };
    poll();
    const timer = setInterval(poll, 250);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [section]);

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

  const handleRunPipelineJob = async () => {
    setBusy(true);
    setOpError(null);
    try {
      const report = await invoke<PipelineJobReport>(
        "run_engine_pipeline_job_cmd",
        {
          mode: pipelineMode,
          payloadSizeMb: payloadMb,
        }
      );
      setPipelineReport(report);
      const snap = await invoke<ProgressSnapshot>("get_live_telemetry_cmd");
      setTelemetry(snap);
      onRefreshStatus();
    } catch (err) {
      setOpError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handlePause = async () => {
    const snap = await invoke<ProgressSnapshot>("pause_job_cmd");
    setTelemetry(snap);
  };

  const handleResume = async () => {
    const snap = await invoke<ProgressSnapshot>("resume_job_cmd");
    setTelemetry(snap);
  };

  const handleCancel = async () => {
    const snap = await invoke<ProgressSnapshot>("cancel_job_cmd");
    setTelemetry(snap);
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
                    <td>{entry.checksum_hex || "-"}</td>
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
      <div style={{ display: "flex", flexDirection: "column", gap: "18px" }}>
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Activity size={16} /> Live Operation Progress &amp; Telemetry
            </span>
            <span className="badge badge-accent">
              State: {telemetry ? telemetry.state : "Idle"}
            </span>
          </div>

          <div style={{ marginBottom: "14px" }}>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                fontSize: "13px",
                marginBottom: "6px",
              }}
            >
              <span>
                <strong>{telemetry?.current_operation || "Ready"}</strong> —{" "}
                {telemetry?.current_file || "No active file"}
              </span>
              <strong>{telemetry ? telemetry.progress_percent : 0}%</strong>
            </div>
            <div className="progress-track" style={{ height: "12px" }}>
              <div
                className="progress-fill"
                style={{
                  width: `${telemetry ? telemetry.progress_percent : 0}%`,
                }}
              />
            </div>
          </div>

          <div className="grid-3" style={{ marginBottom: "16px" }}>
            <div className="card" style={{ padding: "14px" }}>
              <div className="metric-row">
                <span>Processing Speed</span>
                <strong>
                  {telemetry ? telemetry.throughput_mb_s.toFixed(2) : "0.00"} MB/s
                </strong>
              </div>
              <div className="metric-row">
                <span>Compression Ratio</span>
                <strong>
                  {telemetry
                    ? telemetry.compression_ratio_percent.toFixed(1)
                    : "100.0"}
                  %
                </strong>
              </div>
              <div className="metric-row">
                <span>Processed Volume</span>
                <strong>
                  {telemetry
                    ? (telemetry.processed_bytes / (1024 * 1024)).toFixed(2)
                    : "0.00"}{" "}
                  MB
                </strong>
              </div>
            </div>

            <div className="card" style={{ padding: "14px" }}>
              <div className="metric-row">
                <span>Elapsed Time</span>
                <strong>{telemetry ? telemetry.elapsed_ms : 0} ms</strong>
              </div>
              <div className="metric-row">
                <span>Estimated Remaining</span>
                <strong>
                  {telemetry ? telemetry.estimated_remaining_ms : 0} ms
                </strong>
              </div>
              <div className="metric-row">
                <span>Active Worker Threads</span>
                <strong>
                  {telemetry ? telemetry.active_threads : 0} /{" "}
                  {status.config.worker_threads}
                </strong>
              </div>
            </div>

            <div className="card" style={{ padding: "14px" }}>
              <div className="metric-row">
                <span>System CPU Load</span>
                <strong>
                  {telemetry ? telemetry.cpu_usage_percent.toFixed(1) : "0.0"}%
                </strong>
              </div>
              <div className="metric-row">
                <span>Bounded Arena Peak</span>
                <strong>
                  {telemetry ? telemetry.arena_peak_mb.toFixed(2) : "0.00"} MB /{" "}
                  {status.config.memory_arena_mb} MB
                </strong>
              </div>
              <div className="metric-row">
                <span>System RAM Used</span>
                <strong>
                  {telemetry ? telemetry.memory_used_mb.toFixed(0) : "0"} MB
                </strong>
              </div>
            </div>
          </div>

          <div style={{ display: "flex", gap: "10px" }}>
            <button className="btn" onClick={handlePause}>
              <Pause size={14} /> Pause Job
            </button>
            <button className="btn" onClick={handleResume}>
              <RotateCcw size={14} /> Resume Job
            </button>
            <button
              className="btn"
              onClick={handleCancel}
              style={{ borderColor: "var(--status-danger)" }}
            >
              <Square size={14} /> Cancel Job
            </button>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Cpu size={16} /> Kinetic Engine Multithreaded Pipeline Runner
            </span>
            <span className="badge badge-success">
              XXH3-64 + BLAKE3 Stream Verification
            </span>
          </div>

          <div className="grid-2">
            <div className="form-group">
              <label className="form-label">Pipeline Compression Mode</label>
              <select
                className="form-select"
                value={pipelineMode}
                onChange={(e) =>
                  setPipelineMode(e.target.value as CompressionMode)
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
                Verification Stream Size ({payloadMb} MB)
              </label>
              <input
                type="number"
                className="form-input"
                min={4}
                max={128}
                step={4}
                value={payloadMb}
                onChange={(e) =>
                  setPayloadMb(Math.max(4, Number(e.target.value) || 16))
                }
              />
            </div>
          </div>

          <button
            className="btn btn-primary"
            onClick={handleRunPipelineJob}
            disabled={busy}
          >
            <Play size={15} />{" "}
            {busy
              ? "Running Multithreaded Pipeline..."
              : `Run ${status.config.worker_threads}-Thread Pipeline Verification`}
          </button>

          {opError && (
            <div
              className="card"
              style={{ marginTop: "14px", borderColor: "var(--status-danger)" }}
            >
              <strong>Pipeline Error:</strong> {opError}
            </div>
          )}

          {pipelineReport && (
            <div
              className="card"
              style={{ marginTop: "14px", borderColor: "var(--status-success)" }}
            >
              <div className="metric-row">
                <span>Blocks Processed in Parallel</span>
                <strong>
                  {pipelineReport.total_blocks} Independent Blocks (
                  {pipelineReport.worker_threads_used} Threads)
                </strong>
              </div>
              <div className="metric-row">
                <span>Measured Throughput</span>
                <strong>{pipelineReport.throughput_mb_s.toFixed(2)} MB/s</strong>
              </div>
              <div className="metric-row">
                <span>Compressed Size &amp; Ratio</span>
                <strong>
                  {(
                    pipelineReport.total_compressed_bytes /
                    (1024 * 1024)
                  ).toFixed(2)}{" "}
                  MB ({pipelineReport.compression_ratio_percent.toFixed(1)}%)
                </strong>
              </div>
              <div className="metric-row">
                <span>BLAKE3 Full-Stream Root Hash</span>
                <strong style={{ fontFamily: "monospace", fontSize: "12px" }}>
                  {pipelineReport.blake3_root_hash_hex}
                </strong>
              </div>
            </div>
          )}
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