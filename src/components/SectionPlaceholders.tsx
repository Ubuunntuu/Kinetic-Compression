import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  FilePlus2,
  FolderOutput,
  Activity,
  Gauge,
  ShieldCheck,
  Play,
  Pause,
  RotateCcw,
  Square,
  Cpu,
  Lock,
  UploadCloud,
  Sliders,
} from "lucide-react";
import {
  ArchiveFormat,
  CompressionMode,
  EngineStatusPayload,
  NavSection,
  OperationResult,
  PipelineJobReport,
  ProgressSnapshot,
} from "../types/archive";
import { ArchiveExplorer } from "./ArchiveExplorer";

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
  const [threadCount, setThreadCount] = useState<number>(
    status.config.worker_threads
  );
  const [dictionarySizeKb, setDictionarySizeKb] = useState<number>(64);
  const [solidBlocks, setSolidBlocks] = useState<boolean>(true);
  const [fastCdcDedup, setFastCdcDedup] = useState<boolean>(
    status.config.enable_fastcdc_dedup
  );
  const [kineticShield, setKineticShield] = useState<boolean>(
    status.config.enable_kinetic_shield_parity
  );
  const [createPassword, setCreatePassword] = useState<string>("");
  const [dragActive, setDragActive] = useState<boolean>(false);

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
  const [extractPassword, setExtractPassword] = useState<string>("");

  const [telemetry, setTelemetry] = useState<ProgressSnapshot | null>(null);
  const [throughputHistory, setThroughputHistory] = useState<number[]>(
    Array(24).fill(0)
  );
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
          if (active) {
            setTelemetry(snap);
            setThroughputHistory((prev) => [
              ...prev.slice(1),
              snap.throughput_mb_s,
            ]);
          }
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
      await invoke("update_user_config", {
        newConfig: {
          ...status.config,
          worker_threads: threadCount,
          enable_fastcdc_dedup: fastCdcDedup,
          enable_kinetic_shield_parity: kineticShield,
        },
      });

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
          password: createPassword.trim().length > 0 ? createPassword : null,
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
          password: extractPassword.trim().length > 0 ? extractPassword : null,
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
      setThroughputHistory((prev) => [
        ...prev.slice(1),
        report.throughput_mb_s,
      ]);
      onRefreshStatus();
    } catch (err) {
      setOpError(String(err));
    } finally {
      setBusy(false);
    }
  };

  if (section === "explorer") {
    return (
      <ArchiveExplorer
        defaultOverwritePolicy={status.config.overwrite_policy}
        onRefreshStatus={onRefreshStatus}
      />
    );
  }

  if (section === "create") {
    return (
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <FilePlus2 size={16} /> Create Archive (Advanced Compression
            Configurator)
          </span>
          <span className="badge badge-accent">
            Default: .zip • Native: .kinetic v1.0
          </span>
        </div>

        {/* Drag and Drop Files/Folders Zone */}
        <div
          className={`dropzone ${dragActive ? "drag-active" : ""}`}
          onDragOver={(e) => {
            e.preventDefault();
            setDragActive(true);
          }}
          onDragLeave={() => setDragActive(false)}
          onDrop={(e) => {
            e.preventDefault();
            setDragActive(false);
            if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
              const droppedPaths: string[] = [];
              for (let i = 0; i < e.dataTransfer.files.length; i++) {
                const f = e.dataTransfer.files[i] as File & { path?: string };
                if (f.path) droppedPaths.push(f.path);
              }
              if (droppedPaths.length > 0) {
                setSourceInput(droppedPaths.join("; "));
              }
            }
          }}
        >
          <UploadCloud size={20} style={{ marginBottom: "4px" }} />
          <div>
            Drag and drop files or folders here to add them to the compression
            queue
          </div>
        </div>

        <div className="grid-2">
          <div className="form-group">
            <label className="form-label">
              Source Files or Folders (Semicolon ; separated)
            </label>
            <input
              type="text"
              className="form-input"
              value={sourceInput}
              onChange={(e) => setSourceInput(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">Output Destination Path</label>
            <input
              type="text"
              className="form-input"
              value={outputPath}
              onChange={(e) => setOutputPath(e.target.value)}
            />
          </div>
        </div>

        <div className="grid-3">
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
            <label className="form-label">Compression Level / Mode</label>
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

          <div className="form-group">
            <label className="form-label">
              Worker Thread Count (1 to {status.hardware.logical_cores})
            </label>
            <input
              type="number"
              className="form-input"
              min={1}
              max={status.hardware.logical_cores}
              value={threadCount}
              onChange={(e) =>
                setThreadCount(
                  Math.max(
                    1,
                    Math.min(
                      status.hardware.logical_cores,
                      Number(e.target.value) || 1
                    )
                  )
                )
              }
            />
          </div>
        </div>

        <div className="grid-3">
          <div className="form-group">
            <label className="form-label">
              <Sliders size={13} /> Seed Dictionary / Window Size
            </label>
            <select
              className="form-select"
              value={dictionarySizeKb}
              onChange={(e) => setDictionarySizeKb(Number(e.target.value))}
            >
              <option value={64}>64 KB Auto-Trained Seed Dictionary</option>
              <option value={4096}>4 MB Block Dictionary Window</option>
              <option value={16384}>16 MB Micro-Solid Window</option>
              <option value={32768}>32 MB Ultra Archival Window</option>
            </select>
          </div>

          <div className="form-group">
            <label className="form-label">
              <Lock size={13} /> AES-256-GCM + Argon2id Password
            </label>
            <input
              type="password"
              className="form-input"
              placeholder="Optional encryption password..."
              value={createPassword}
              onChange={(e) => setCreatePassword(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">Advanced Kinetic Options</label>
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                gap: "4px",
                fontSize: "12.5px",
              }}
            >
              <label style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <input
                  type="checkbox"
                  checked={solidBlocks}
                  onChange={(e) => setSolidBlocks(e.target.checked)}
                />
                <span>16 MB Indexed Micro-Solid Grouping</span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <input
                  type="checkbox"
                  checked={fastCdcDedup}
                  onChange={(e) => setFastCdcDedup(e.target.checked)}
                />
                <span>FastCDC Global Chunk Deduplication</span>
              </label>
              <label style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <input
                  type="checkbox"
                  checked={kineticShield}
                  onChange={(e) => setKineticShield(e.target.checked)}
                />
                <span>Kinetic Shield Parity Recovery Blocks</span>
              </label>
            </div>
          </div>
        </div>

        <button
          className="btn btn-primary"
          onClick={handleCreateArchive}
          disabled={busy}
        >
          <Play size={15} />{" "}
          {busy ? "Compressing..." : "Start Multithreaded Compression"}
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
            <ShieldCheck size={13} /> Zip-Slip, BLAKE3 &amp; Parity Self-Healing
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

        <div className="grid-3">
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

          <div className="form-group">
            <label className="form-label">Decryption Password (If Encrypted)</label>
            <input
              type="password"
              className="form-input"
              placeholder="Optional password..."
              value={extractPassword}
              onChange={(e) => setExtractPassword(e.target.value)}
            />
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

  if (section === "performance") {
    const maxSpeed = Math.max(...throughputHistory, 100);

    return (
      <div style={{ display: "flex", flexDirection: "column", gap: "18px" }}>
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Activity size={16} /> Real-Time Performance Monitor &amp;
              Throughput Graph
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

          {/* Rolling 24-Sample Throughput Sparkline Chart */}
          <div style={{ marginBottom: "16px" }}>
            <div
              style={{
                fontSize: "12px",
                color: "var(--text-secondary)",
                marginBottom: "6px",
              }}
            >
              Rolling Throughput Telemetry (Peak Scale: {maxSpeed.toFixed(0)}{" "}
              MB/s)
            </div>
            <div className="telemetry-bars">
              {throughputHistory.map((val, idx) => (
                <div
                  key={idx}
                  className="telemetry-bar"
                  style={{
                    height: `${Math.max(6, Math.min(100, (val / maxSpeed) * 100))}%`,
                  }}
                  title={`${val.toFixed(1)} MB/s`}
                />
              ))}
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
            <button
              className="btn"
              onClick={async () =>
                setTelemetry(await invoke<ProgressSnapshot>("pause_job_cmd"))
              }
            >
              <Pause size={14} /> Pause Job
            </button>
            <button
              className="btn"
              onClick={async () =>
                setTelemetry(await invoke<ProgressSnapshot>("resume_job_cmd"))
              }
            >
              <RotateCcw size={14} /> Resume Job
            </button>
            <button
              className="btn"
              onClick={async () =>
                setTelemetry(await invoke<ProgressSnapshot>("cancel_job_cmd"))
              }
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