import React from "react";
import {
  FilePlus2,
  FolderOutput,
  HardDrive,
  Cpu,
  History,
  Archive,
  Zap,
  ShieldCheck,
} from "lucide-react";
import { EngineStatusPayload, NavSection } from "../types/archive";

interface DashboardProps {
  status: EngineStatusPayload;
  onNavigate: (section: NavSection) => void;
  onClearHistory: () => void;
}

export const Dashboard: React.FC<DashboardProps> = ({
  status,
  onNavigate,
  onClearHistory,
}) => {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
      <div className="grid-2">
        <div className="action-card" onClick={() => onNavigate("create")}>
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: "10px", marginBottom: "6px" }}>
              <FilePlus2 size={20} color="var(--accent-primary)" />
              <span style={{ fontSize: "16px", fontWeight: 600 }}>Quick Compression</span>
            </div>
            <p style={{ color: "var(--text-secondary)", fontSize: "13px" }}>
              Create a new <strong>{status.default_format.toUpperCase()}</strong> archive using{" "}
              <strong>{status.default_mode}</strong> mode ({status.config.worker_threads} threads)
            </p>
          </div>
          <span className="badge badge-accent">Launch →</span>
        </div>

        <div className="action-card" onClick={() => onNavigate("extract")}>
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: "10px", marginBottom: "6px" }}>
              <FolderOutput size={20} color="var(--status-success)" />
              <span style={{ fontSize: "16px", fontWeight: 600 }}>Quick Extraction</span>
            </div>
            <p style={{ color: "var(--text-secondary)", fontSize: "13px" }}>
              Extract or verify <strong>.zip</strong>, <strong>.kinetic</strong>,{" "}
              <strong>.7z</strong>, or <strong>.rar</strong> archives with Zip-Slip protection
            </p>
          </div>
          <span className="badge badge-success">Extract →</span>
        </div>
      </div>

      <div className="grid-3">
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Cpu size={15} /> Engine & Hardware Readiness
            </span>
            <span className="badge badge-success">Online</span>
          </div>
          <div className="metric-row">
            <span>Engine Version</span>
            <strong>{status.engine_name} v{status.engine_version}</strong>
          </div>
          <div className="metric-row">
            <span>CPU Logical Cores</span>
            <strong>{status.hardware.logical_cores} Cores</strong>
          </div>
          <div className="metric-row">
            <span>Configured Worker Pool</span>
            <strong>{status.config.worker_threads} Threads</strong>
          </div>
          <div className="metric-row">
            <span>Bounded Memory Arena</span>
            <strong>{status.config.memory_arena_mb} MB Cap</strong>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Zap size={15} /> Active Engine Pipeline
            </span>
            <span className="badge badge-accent">{status.default_mode}</span>
          </div>
          <div className="metric-row">
            <span>Default Archive Format</span>
            <strong>{status.default_format} ({status.supported_formats.find(f => f.format === status.default_format)?.extension})</strong>
          </div>
          <div className="metric-row">
            <span>FastCDC Global Dedup</span>
            <strong>{status.config.enable_fastcdc_dedup ? "Enabled" : "Disabled"}</strong>
          </div>
          <div className="metric-row">
            <span>Shannon Entropy Media Bypass</span>
            <strong>{status.config.enable_entropy_bypass ? "Enabled (H > 7.85)" : "Disabled"}</strong>
          </div>
          <div className="metric-row">
            <span>Kinetic Shield Parity Recovery</span>
            <strong>{status.config.enable_kinetic_shield_parity ? "Active" : "Optional"}</strong>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <HardDrive size={15} /> Local Storage Statistics
            </span>
            <span className="badge">{status.disks.length} Volumes</span>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
            {status.disks.map((disk) => (
              <div key={disk.mount_point}>
                <div style={{ display: "flex", justifyContent: "space-between", fontSize: "12.5px" }}>
                  <strong>
                    {disk.mount_point} ({disk.file_system || "NTFS"})
                  </strong>
                  <span>
                    {disk.available_gb} GB free of {disk.total_gb} GB ({disk.used_percent}%)
                  </span>
                </div>
                <div className="progress-track">
                  <div
                    className="progress-fill"
                    style={{ width: `${Math.min(100, disk.used_percent)}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Archive size={15} /> Recent Archives
            </span>
            {status.recent_archives.length > 0 && (
              <button className="btn" onClick={onClearHistory} style={{ padding: "4px 8px", fontSize: "11px" }}>
                Clear
              </button>
            )}
          </div>
          {status.recent_archives.length === 0 ? (
            <div className="empty-state">
              No archives opened or created in this session yet. Use <strong>Create Archive</strong> or{" "}
              <strong>Archive Explorer</strong> to begin.
            </div>
          ) : (
            <table className="data-table">
              <thead>
                <tr>
                  <th>Archive</th>
                  <th>Format</th>
                  <th>Size</th>
                </tr>
              </thead>
              <tbody>
                {status.recent_archives.map((item) => (
                  <tr key={item.path}>
                    <td>{item.name}</td>
                    <td>{item.format}</td>
                    <td>{(item.size_bytes / (1024 * 1024)).toFixed(2)} MB</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <History size={15} /> Recent Operations & Performance
            </span>
            <span className="badge">
              <ShieldCheck size={13} /> Verified Metrics Only
            </span>
          </div>
          {status.recent_operations.length === 0 ? (
            <div className="empty-state">
              No compression or extraction jobs executed yet. Real throughput (MB/s) and compression
              ratios will appear here after your first operation.
            </div>
          ) : (
            <table className="data-table">
              <thead>
                <tr>
                  <th>Operation</th>
                  <th>Target</th>
                  <th>Speed</th>
                  <th>Ratio</th>
                </tr>
              </thead>
              <tbody>
                {status.recent_operations.map((op) => (
                  <tr key={op.id}>
                    <td>{op.operation_type}</td>
                    <td>{op.target_name}</td>
                    <td>{op.throughput_mb_s.toFixed(1)} MB/s</td>
                    <td>{op.compression_ratio_percent.toFixed(1)}%</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  );
};