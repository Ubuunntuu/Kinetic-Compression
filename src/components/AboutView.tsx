import React from "react";
import { Shield, Cpu, Layers } from "lucide-react";
import { EngineStatusPayload } from "../types/archive";

interface AboutViewProps {
  status: EngineStatusPayload;
}

export const AboutView: React.FC<AboutViewProps> = ({ status }) => {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "18px" }}>
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <Cpu size={16} /> About Kinetic & Kinetic Engine
          </span>
          <span className="badge badge-accent">v{status.engine_version}</span>
        </div>
        <p style={{ color: "var(--text-secondary)", marginBottom: "14px" }}>
          <strong>Kinetic</strong> is a high-performance Windows compression and archive management
          application powered by the independent <strong>Kinetic Engine</strong> in Rust and Tauri 2.
        </p>
        <div className="metric-row">
          <span>Engine Architecture</span>
          <strong>Rust Multithreaded Ring-Buffer Pipeline + Tauri 2 IPC</strong>
        </div>
        <div className="metric-row">
          <span>Default Archive Format</span>
          <strong>.zip (Universal Compatibility)</strong>
        </div>
        <div className="metric-row">
          <span>Native Archive Format</span>
          <strong>.kinetic v1.0 (64-Byte Header, Block-Multiplexed, FastCDC, BLAKE3)</strong>
        </div>
        <div className="metric-row">
          <span>Authenticated Encryption</span>
          <strong>AES-256-GCM + Argon2id Key Derivation</strong>
        </div>
      </div>

      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Layers size={15} /> 11-Pillar Kinetic Engine Architecture
            </span>
          </div>
          <div className="metric-row">
            <span>1. Content-Defined Deduplication</span>
            <strong>FastCDC Gear Hash</strong>
          </div>
          <div className="metric-row">
            <span>2. Per-Block Codec Multiplexing</span>
            <strong>Store / LZ4 / Zstd / LZMA2 / Brotli</strong>
          </div>
          <div className="metric-row">
            <span>3. Incompressible Media Bypass</span>
            <strong>Shannon Entropy H &gt; 7.85</strong>
          </div>
          <div className="metric-row">
            <span>4. Executable & Structured Filters</span>
            <strong>x86/ARM64 BCJ + Delta + Byte-Shuffle</strong>
          </div>
          <div className="metric-row">
            <span>5. Micro-Solid & Seed Dictionaries</span>
            <strong>16 MB Clusters + 64 KB Auto-Dict</strong>
          </div>
          <div className="metric-row">
            <span>6. Kinetic Shield Self-Healing</span>
            <strong>SIMD Parity Recovery Blocks</strong>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Shield size={15} /> Format Compatibility & Licensing Compliance
            </span>
          </div>
          {status.supported_formats.map((f) => (
            <div className="metric-row" key={f.format}>
              <span>
                <strong>{f.format}</strong> ({f.extension})
              </span>
              <span>
                {f.can_create
                  ? "Full Native Create, Extract, Browse & Verify"
                  : "Extract & Browse Only (WinRAR License Compliant)"}
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};