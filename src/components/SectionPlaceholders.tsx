import React from "react";
import {
  FilePlus2,
  FolderOutput,
  FolderTree,
  Activity,
  Gauge,
} from "lucide-react";
import { EngineStatusPayload, NavSection } from "../types/archive";

interface SectionWorkspaceProps {
  section: NavSection;
  status: EngineStatusPayload;
}

export const SectionWorkspace: React.FC<SectionWorkspaceProps> = ({
  section,
  status,
}) => {
  if (section === "create") {
    return (
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <FilePlus2 size={16} /> Create Archive Workspace
          </span>
          <span className="badge badge-accent">
            Default: {status.default_format} ({status.default_mode})
          </span>
        </div>
        <div className="grid-2">
          <div className="form-group">
            <label className="form-label">Target Archive Format</label>
            <select className="form-select" defaultValue={status.default_format}>
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
            <select className="form-select" defaultValue={status.default_mode}>
              {status.compression_modes.map((m) => (
                <option key={m.mode} value={m.mode}>
                  {m.mode} ({Math.round(m.target_block_size_bytes / (1024 * 1024))} MB Blocks)
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="empty-state">
          UI Foundation Ready — Full multithreaded file/folder selection and compression streaming
          pipeline connects in <strong>Phase 3 &amp; Phase 4</strong>.
        </div>
      </div>
    );
  }

  if (section === "extract") {
    return (
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <FolderOutput size={16} /> Extract Archive Workspace
          </span>
          <span className="badge badge-success">Supports .zip, .kinetic, .7z, .rar</span>
        </div>
        <div className="empty-state">
          UI Foundation Ready — Native extraction providers with Zip-Slip path traversal guards
          connect in <strong>Phase 3</strong>.
        </div>
      </div>
    );
  }

  if (section === "explorer") {
    return (
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <FolderTree size={16} /> Archive Explorer
          </span>
          <span className="badge">Virtualized Tree View</span>
        </div>
        <div className="empty-state">
          UI Foundation Ready — Archive entry browser, search, sorting, and drag-and-drop connect
          in <strong>Phase 3 &amp; Phase 7</strong>.
        </div>
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
            {status.hardware.logical_cores} Cores • {status.hardware.available_memory_mb} MB Free RAM
          </span>
        </div>
        <div className="empty-state">
          UI Foundation Ready — Live atomic worker thread telemetry, memory arena usage, and
          pause/resume/cancel controls connect in <strong>Phase 4 &amp; Phase 7</strong>.
        </div>
      </div>
    );
  }

  return (
    <div className="card">
      <div className="card-header">
        <span className="card-title">
          <Gauge size={16} /> Reproducible Benchmark Center (vs. 7-Zip &amp; WinRAR)
        </span>
        <span className="badge badge-accent">No Fabricated Metrics</span>
      </div>
      <div className="empty-state">
        UI Foundation Ready — Automated comparative benchmark harness against local 7-Zip and
        WinRAR binaries connects in <strong>Phase 9</strong>.
      </div>
    </div>
  );
};