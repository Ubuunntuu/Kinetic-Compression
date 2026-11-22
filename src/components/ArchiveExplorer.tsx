import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  FolderTree,
  Folder,
  FileText,
  Search,
  ShieldCheck,
  Download,
  Copy,
  CheckSquare,
  Square,
  ArrowUpDown,
  ChevronRight,
  UploadCloud,
} from "lucide-react";
import {
  ArchiveEntry,
  ArchiveInspection,
  EntryPreview,
  IntegrityReport,
  OperationResult,
} from "../types/archive";

interface ArchiveExplorerProps {
  defaultOverwritePolicy: string;
  onRefreshStatus: () => void;
}

type SortField = "name" | "size" | "compressed" | "type" | "codec" | "modified";

export const ArchiveExplorer: React.FC<ArchiveExplorerProps> = ({
  defaultOverwritePolicy,
  onRefreshStatus,
}) => {
  const [archivePath, setArchivePath] = useState<string>(
    "C:\\projects\\kinetic compression\\bench_output\\sample.zip"
  );
  const [password, setPassword] = useState<string>("");
  const [inspection, setInspection] = useState<ArchiveInspection | null>(null);
  const [validation, setValidation] = useState<IntegrityReport | null>(null);
  const [extractResult, setExtractResult] = useState<OperationResult | null>(
    null
  );
  const [previewData, setPreviewData] = useState<EntryPreview | null>(null);
  const [previewMode, setPreviewMode] = useState<"text" | "hex">("text");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);

  const [currentFolder, setCurrentFolder] = useState<string>("");
  const [flatView, setFlatView] = useState<boolean>(false);
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [sortField, setSortField] = useState<SortField>("name");
  const [sortAsc, setSortAsc] = useState<boolean>(true);
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
  const [extractTargetDir, setExtractTargetDir] = useState<string>(
    "C:\\projects\\kinetic compression\\bench_output\\selected_extracted"
  );
  const [dragActive, setDragActive] = useState<boolean>(false);

  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    entry: ArchiveEntry;
  } | null>(null);

  useEffect(() => {
    const closeMenu = () => setContextMenu(null);
    window.addEventListener("click", closeMenu);
    return () => window.removeEventListener("click", closeMenu);
  }, []);

  const loadArchive = async (targetPath?: string) => {
    const pathToLoad = targetPath || archivePath;
    setBusy(true);
    setError(null);
    setValidation(null);
    setExtractResult(null);
    try {
      const res = await invoke<ArchiveInspection>("inspect_archive_cmd", {
        archivePath: pathToLoad,
        password: password.trim().length > 0 ? password : null,
      });
      setInspection(res);
      setCurrentFolder("");
      setSelectedPaths(new Set());
      onRefreshStatus();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handlePreviewEntry = async (entryPath: string) => {
    setBusy(true);
    setError(null);
    try {
      const res = await invoke<EntryPreview>("preview_entry_cmd", {
        archivePath,
        entryPath,
        password: password.trim().length > 0 ? password : null,
      });
      setPreviewData(res);
      setPreviewMode(res.is_text ? "text" : "hex");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleDeleteEntry = async (entryPath: string) => {
    setBusy(true);
    setError(null);
    try {
      const msg = await invoke<string>("delete_entry_cmd", {
        archivePath,
        entryPath,
        password: password.trim().length > 0 ? password : null,
      });
      setValidation({
        valid: true,
        verified_entries: 0,
        total_uncompressed_bytes: 0,
        message: msg,
      });
      await loadArchive();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleSecurityAudit = async () => {
    setBusy(true);
    setError(null);
    try {
      const report = await invoke<IntegrityReport>("audit_archive_cmd", {
        archivePath,
        password: password.trim().length > 0 ? password : null,
      });
      setValidation(report);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRepairArchive = async () => {
    setBusy(true);
    setError(null);
    try {
      const report = await invoke<IntegrityReport>("repair_archive_cmd", {
        archivePath,
        password: password.trim().length > 0 ? password : null,
      });
      setValidation(report);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleValidate = async () => {
    setBusy(true);
    setError(null);
    try {
      const report = await invoke<IntegrityReport>("validate_archive_cmd", {
        archivePath,
        password: password.trim().length > 0 ? password : null,
      });
      setValidation(report);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleExtractSelected = async (pathsOverride?: string[]) => {
    const targets = pathsOverride || Array.from(selectedPaths);
    setBusy(true);
    setError(null);
    setExtractResult(null);
    try {
      const res = await invoke<OperationResult>("extract_selected_cmd", {
        request: {
          archive_path: archivePath,
          destination_path: extractTargetDir,
          selected_paths: targets,
          overwrite_policy: defaultOverwritePolicy,
          password: password.trim().length > 0 ? password : null,
        },
      });
      setExtractResult(res);
      onRefreshStatus();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const getFileType = (entry: ArchiveEntry): string => {
    if (entry.is_dir) return "Folder";
    const parts = entry.path.split(".");
    if (parts.length > 1) {
      return `${parts[parts.length - 1].toUpperCase()} File`;
    }
    return "Binary File";
  };

  const visibleEntries = useMemo(() => {
    if (!inspection) return [];
    const query = searchQuery.trim().toLowerCase();

    let list: ArchiveEntry[] = [];
    if (flatView || query.length > 0) {
      list = inspection.entries.filter((e) =>
        e.path.toLowerCase().includes(query)
      );
    } else {
      const prefix = currentFolder ? `${currentFolder}/` : "";
      const seenFolders = new Set<string>();

      for (const entry of inspection.entries) {
        const norm = entry.path.replace(/\\/g, "/");
        if (!norm.startsWith(prefix)) continue;
        const remainder = norm.slice(prefix.length);
        if (!remainder) continue;

        const slashIdx = remainder.indexOf("/");
        if (slashIdx === -1) {
          list.push(entry);
        } else {
          const subfolderName = remainder.slice(0, slashIdx);
          const virtualPath = prefix + subfolderName;
          if (!seenFolders.has(virtualPath)) {
            seenFolders.add(virtualPath);
            list.push({
              path: virtualPath,
              is_dir: true,
              uncompressed_size: 0,
              compressed_size: 0,
              modified_unix_secs: 0,
              checksum_hex: null,
              codec_name: "Directory",
            });
          }
        }
      }
    }

    return [...list].sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      let cmp = 0;
      if (sortField === "name") cmp = a.path.localeCompare(b.path);
      else if (sortField === "size")
        cmp = a.uncompressed_size - b.uncompressed_size;
      else if (sortField === "compressed")
        cmp = a.compressed_size - b.compressed_size;
      else if (sortField === "type")
        cmp = getFileType(a).localeCompare(getFileType(b));
      else if (sortField === "codec")
        cmp = a.codec_name.localeCompare(b.codec_name);
      else if (sortField === "modified")
        cmp = a.modified_unix_secs - b.modified_unix_secs;
      return sortAsc ? cmp : -cmp;
    });
  }, [inspection, currentFolder, flatView, searchQuery, sortField, sortAsc]);

  const toggleSort = (field: SortField) => {
    if (sortField === field) setSortAsc(!sortAsc);
    else {
      setSortField(field);
      setSortAsc(true);
    }
  };

  const toggleSelect = (path: string) => {
    const next = new Set(selectedPaths);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    setSelectedPaths(next);
  };

  const selectAllVisible = () => {
    if (selectedPaths.size === visibleEntries.length) {
      setSelectedPaths(new Set());
    } else {
      setSelectedPaths(new Set(visibleEntries.map((e) => e.path)));
    }
  };

  const breadcrumbs = currentFolder
    ? currentFolder.split("/").filter(Boolean)
    : [];

  return (
    <div className="card">
      <div className="card-header">
        <span className="card-title">
          <FolderTree size={16} /> Archive Explorer (Browse, Preview, Modify,
          Audit &amp; Repair)
        </span>
        {inspection && (
          <span className="badge badge-accent">
            {inspection.format} • {inspection.total_entries} Entries •{" "}
            {(inspection.total_uncompressed_bytes / 1024).toFixed(1)} KB Total
          </span>
        )}
      </div>

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
            const file = e.dataTransfer.files[0] as File & { path?: string };
            if (file.path) {
              setArchivePath(file.path);
              loadArchive(file.path);
            }
          }
        }}
      >
        <UploadCloud size={20} style={{ marginBottom: "4px" }} />
        <div>
          Drag and drop any <strong>.zip</strong>, <strong>.kinetic</strong>,{" "}
          <strong>.7z</strong>, or <strong>.rar</strong> archive here to open in
          Archive Explorer
        </div>
      </div>

      <div
        style={{
          display: "flex",
          gap: "10px",
          marginBottom: "14px",
          alignItems: "center",
          flexWrap: "wrap",
        }}
      >
        <input
          type="text"
          className="form-input"
          style={{ flex: 2, minWidth: "240px" }}
          placeholder="Archive file path..."
          value={archivePath}
          onChange={(e) => setArchivePath(e.target.value)}
        />
        <input
          type="password"
          className="form-input"
          style={{ flex: 1, minWidth: "160px" }}
          placeholder="AES-256-GCM Password (optional)"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
        <button
          className="btn btn-primary"
          onClick={() => loadArchive()}
          disabled={busy}
        >
          <Search size={15} /> Open Archive
        </button>
        <button className="btn" onClick={handleSecurityAudit} disabled={busy}>
          Security Audit
        </button>
        <button className="btn" onClick={handleRepairArchive} disabled={busy}>
          Repair (.kinetic)
        </button>
        <button className="btn" onClick={handleValidate} disabled={busy}>
          <ShieldCheck size={15} /> Verify Integrity
        </button>
      </div>

      {error && (
        <div
          className="card"
          style={{ marginBottom: "14px", borderColor: "var(--status-danger)" }}
        >
          <strong>Explorer Error:</strong> {error}
        </div>
      )}

      {validation && (
        <div
          className="card"
          style={{ marginBottom: "14px", borderColor: "var(--status-success)" }}
        >
          <strong>Operation Report:</strong> {validation.message}
        </div>
      )}

      {extractResult && (
        <div
          className="card"
          style={{ marginBottom: "14px", borderColor: "var(--status-success)" }}
        >
          <strong>Selective Extraction Complete:</strong>{" "}
          {extractResult.message}
        </div>
      )}

      {previewData && (
        <div
          className="card"
          style={{
            marginBottom: "16px",
            borderColor: "var(--accent-primary)",
            backgroundColor: "var(--bg-elevated)",
          }}
        >
          <div className="card-header">
            <span className="card-title">
              <FileText size={15} /> In-Memory Inspector: {previewData.path} (
              {previewData.codec_name} • {previewData.size_bytes} bytes)
            </span>
            <div style={{ display: "flex", gap: "8px" }}>
              <button
                className={`btn ${previewMode === "text" ? "btn-primary" : ""}`}
                onClick={() => setPreviewMode("text")}
                style={{ padding: "4px 10px", fontSize: "12px" }}
              >
                UTF-8 Text / Code
              </button>
              <button
                className={`btn ${previewMode === "hex" ? "btn-primary" : ""}`}
                onClick={() => setPreviewMode("hex")}
                style={{ padding: "4px 10px", fontSize: "12px" }}
              >
                Hex + ASCII Dump
              </button>
              <button
                className="btn"
                onClick={() => setPreviewData(null)}
                style={{ padding: "4px 10px", fontSize: "12px" }}
              >
                Close
              </button>
            </div>
          </div>
          <pre
            style={{
              maxHeight: "240px",
              overflowY: "auto",
              padding: "12px",
              backgroundColor: "var(--bg-base)",
              borderRadius: "6px",
              fontFamily: "Consolas, monospace",
              fontSize: "12px",
              whiteSpace: "pre-wrap",
              userSelect: "text",
            }}
          >
            {previewMode === "text"
              ? previewData.text_preview
              : previewData.hex_preview}
          </pre>
        </div>
      )}

      {inspection && (
        <>
          <div className="breadcrumb-bar">
            <button
              className={`breadcrumb-btn ${currentFolder === "" ? "active" : ""}`}
              onClick={() => setCurrentFolder("")}
            >
              Archive Root
            </button>
            {breadcrumbs.map((seg, idx) => {
              const target = breadcrumbs.slice(0, idx + 1).join("/");
              return (
                <React.Fragment key={target}>
                  <ChevronRight size={13} color="var(--text-muted)" />
                  <button
                    className={`breadcrumb-btn ${
                      currentFolder === target ? "active" : ""
                    }`}
                    onClick={() => setCurrentFolder(target)}
                  >
                    {seg}
                  </button>
                </React.Fragment>
              );
            })}
            <div style={{ marginLeft: "auto", display: "flex", gap: "8px" }}>
              <button
                className="btn"
                style={{ padding: "4px 10px", fontSize: "12px" }}
                onClick={() => setFlatView(!flatView)}
              >
                {flatView ? "Folder Hierarchy View" : "Flat All-Files View"}
              </button>
            </div>
          </div>

          <div
            style={{
              display: "flex",
              gap: "10px",
              marginBottom: "12px",
              alignItems: "center",
            }}
          >
            <input
              type="text"
              className="form-input"
              style={{ flex: 1 }}
              placeholder="Search files & folders inside archive..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
            <input
              type="text"
              className="form-input"
              style={{ flex: 1 }}
              placeholder="Destination folder for selective extraction..."
              value={extractTargetDir}
              onChange={(e) => setExtractTargetDir(e.target.value)}
            />
            <button
              className="btn btn-primary"
              onClick={() => handleExtractSelected()}
              disabled={busy}
            >
              <Download size={14} />{" "}
              {selectedPaths.size > 0
                ? `Extract Selected (${selectedPaths.size})`
                : "Extract All"}
            </button>
          </div>

          <table className="data-table">
            <thead>
              <tr>
                <th style={{ width: "36px" }}>
                  <button
                    onClick={selectAllVisible}
                    style={{
                      background: "transparent",
                      border: "none",
                      color: "var(--text-secondary)",
                      cursor: "pointer",
                    }}
                  >
                    {selectedPaths.size > 0 &&
                    selectedPaths.size === visibleEntries.length ? (
                      <CheckSquare size={15} />
                    ) : (
                      <Square size={15} />
                    )}
                  </button>
                </th>
                <th
                  className="sortable-th"
                  onClick={() => toggleSort("name")}
                >
                  Name <ArrowUpDown size={12} />
                </th>
                <th
                  className="sortable-th"
                  onClick={() => toggleSort("type")}
                >
                  Type <ArrowUpDown size={12} />
                </th>
                <th
                  className="sortable-th"
                  onClick={() => toggleSort("size")}
                >
                  Original Size <ArrowUpDown size={12} />
                </th>
                <th
                  className="sortable-th"
                  onClick={() => toggleSort("compressed")}
                >
                  Compressed <ArrowUpDown size={12} />
                </th>
                <th
                  className="sortable-th"
                  onClick={() => toggleSort("codec")}
                >
                  Codec / Filter <ArrowUpDown size={12} />
                </th>
                <th
                  className="sortable-th"
                  onClick={() => toggleSort("modified")}
                >
                  Modified / Digest <ArrowUpDown size={12} />
                </th>
              </tr>
            </thead>
            <tbody>
              {visibleEntries.map((entry) => {
                const isSelected = selectedPaths.has(entry.path);
                const displayName = flatView
                  ? entry.path
                  : entry.path.split("/").pop() || entry.path;

                return (
                  <tr
                    key={entry.path}
                    className={`explorer-row ${isSelected ? "selected" : ""}`}
                    onClick={() => {
                      if (entry.is_dir && !flatView) {
                        setCurrentFolder(entry.path);
                      } else {
                        toggleSelect(entry.path);
                      }
                    }}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setContextMenu({
                        x: e.clientX,
                        y: e.clientY,
                        entry,
                      });
                    }}
                  >
                    <td
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleSelect(entry.path);
                      }}
                    >
                      {isSelected ? (
                        <CheckSquare size={15} color="var(--accent-primary)" />
                      ) : (
                        <Square size={15} color="var(--text-muted)" />
                      )}
                    </td>
                    <td
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "8px",
                        fontWeight: entry.is_dir ? 600 : 400,
                      }}
                    >
                      {entry.is_dir ? (
                        <Folder size={15} color="var(--accent-primary)" />
                      ) : (
                        <FileText size={15} color="var(--text-secondary)" />
                      )}
                      <span>{displayName}</span>
                    </td>
                    <td>{getFileType(entry)}</td>
                    <td>
                      {entry.is_dir
                        ? "-"
                        : `${(entry.uncompressed_size / 1024).toFixed(2)} KB`}
                    </td>
                    <td>
                      {entry.is_dir
                        ? "-"
                        : `${(entry.compressed_size / 1024).toFixed(2)} KB`}
                    </td>
                    <td>{entry.codec_name}</td>
                    <td style={{ fontFamily: "monospace", fontSize: "12px" }}>
                      {entry.checksum_hex || "Verified"}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </>
      )}

      {contextMenu && (
        <div
          className="context-menu"
          style={{ top: contextMenu.y, left: contextMenu.x }}
        >
          <button
            className="context-item"
            onClick={() => handleExtractSelected([contextMenu.entry.path])}
          >
            <Download size={14} /> Extract "
            {contextMenu.entry.path.split("/").pop()}"
          </button>
          {!contextMenu.entry.is_dir && (
            <button
              className="context-item"
              onClick={() => handlePreviewEntry(contextMenu.entry.path)}
            >
              <FileText size={14} /> Preview in Memory (Text / Hex)
            </button>
          )}
          <button
            className="context-item"
            onClick={() => handleDeleteEntry(contextMenu.entry.path)}
          >
            <Square size={14} /> Delete Entry (Zero-Recompression .kinetic)
          </button>
          <button className="context-item" onClick={handleValidate}>
            <ShieldCheck size={14} /> Verify Archive Integrity
          </button>
          <button
            className="context-item"
            onClick={() =>
              navigator.clipboard?.writeText(contextMenu.entry.path)
            }
          >
            <Copy size={14} /> Copy Relative Entry Path
          </button>
          {contextMenu.entry.checksum_hex && (
            <button
              className="context-item"
              onClick={() =>
                navigator.clipboard?.writeText(
                  contextMenu.entry.checksum_hex || ""
                )
              }
            >
              <Copy size={14} /> Copy Checksum Digest
            </button>
          )}
          <button className="context-item" onClick={selectAllVisible}>
            <CheckSquare size={14} /> Toggle Select All Visible
          </button>
        </div>
      )}
    </div>
  );
};