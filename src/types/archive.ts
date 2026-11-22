export type ArchiveFormat = "Zip" | "Kinetic" | "SevenZ" | "Rar";

export type CompressionMode =
  | "Store"
  | "Fast"
  | "Balanced"
  | "Maximum"
  | "Ultra"
  | "Automatic";

export type NavSection =
  | "dashboard"
  | "create"
  | "extract"
  | "explorer"
  | "compression-settings"
  | "performance"
  | "benchmark"
  | "settings"
  | "about";

export type JobState =
  | "Idle"
  | "Running"
  | "Paused"
  | "Cancelled"
  | "Completed"
  | "Failed";

export interface HardwareProfile {
  logical_cores: number;
  total_memory_mb: number;
  available_memory_mb: number;
  recommended_threads: number;
  max_safe_memory_arena_mb: number;
}

export interface DiskTelemetry {
  mount_point: string;
  name: string;
  file_system: string;
  total_gb: number;
  available_gb: number;
  used_percent: number;
}

export interface RecentArchive {
  name: string;
  path: string;
  format: ArchiveFormat;
  size_bytes: number;
  last_opened_utc: string;
}

export interface RecentOperation {
  id: string;
  operation_type: string;
  target_name: string;
  format: ArchiveFormat;
  mode: CompressionMode;
  throughput_mb_s: number;
  compression_ratio_percent: number;
  duration_ms: number;
  status: string;
}

export interface UserConfig {
  theme: "dark" | "light";
  default_format: ArchiveFormat;
  default_mode: CompressionMode;
  worker_threads: number;
  memory_arena_mb: number;
  enable_fastcdc_dedup: boolean;
  enable_entropy_bypass: boolean;
  enable_kinetic_shield_parity: boolean;
  verify_integrity_on_complete: boolean;
  overwrite_policy: string;
}

export interface FormatCapability {
  format: ArchiveFormat;
  extension: string;
  can_create: boolean;
  can_extract: boolean;
}

export interface ModeCapability {
  mode: CompressionMode;
  target_block_size_bytes: number;
  description: string;
}

export interface ArchiveEntry {
  path: string;
  is_dir: boolean;
  uncompressed_size: number;
  compressed_size: number;
  modified_unix_secs: number;
  checksum_hex: string | null;
  codec_name: string;
}

export interface ArchiveInspection {
  path: string;
  format: ArchiveFormat;
  archive_size_bytes: number;
  total_entries: number;
  total_uncompressed_bytes: number;
  entries: ArchiveEntry[];
}

export interface IntegrityReport {
  valid: boolean;
  verified_entries: number;
  total_uncompressed_bytes: number;
  message: string;
}

export interface EntryPreview {
  path: string;
  size_bytes: number;
  is_text: boolean;
  text_preview: string;
  hex_preview: string;
  codec_name: string;
}

export interface FsNode {
  name: string;
  path: string;
  is_dir: boolean;
  size_bytes: number;
}

export interface OperationResult {
  success: boolean;
  message: string;
  archive_path: string;
  processed_bytes: number;
  compressed_bytes: number;
  duration_ms: number;
  throughput_mb_s: number;
  compression_ratio_percent: number;
}

export interface ProgressSnapshot {
  state: JobState;
  current_operation: string;
  current_file: string;
  processed_bytes: number;
  total_bytes: number;
  compressed_bytes: number;
  progress_percent: number;
  compression_ratio_percent: number;
  throughput_mb_s: number;
  elapsed_ms: number;
  estimated_remaining_ms: number;
  active_threads: number;
  cpu_usage_percent: number;
  memory_used_mb: number;
  arena_used_mb: number;
  arena_peak_mb: number;
}

export interface PipelineJobReport {
  total_blocks: number;
  total_uncompressed_bytes: number;
  total_compressed_bytes: number;
  compression_ratio_percent: number;
  throughput_mb_s: number;
  duration_ms: number;
  blake3_root_hash_hex: string;
  entropy_bypassed_blocks: number;
  worker_threads_used: number;
  peak_arena_mb: number;
}

export interface EngineStatusPayload {
  engine_name: string;
  engine_version: string;
  default_format: ArchiveFormat;
  default_mode: CompressionMode;
  config: UserConfig;
  hardware: HardwareProfile;
  disks: DiskTelemetry[];
  recent_archives: RecentArchive[];
  recent_operations: RecentOperation[];
  supported_formats: FormatCapability[];
  compression_modes: ModeCapability[];
}
export interface BenchmarkEntryResult {
  dataset_name: string;
  engine_label: string;
  original_bytes: number;
  compressed_bytes: number;
  compression_ratio_percent: number;
  compress_ms: number;
  extract_ms: number;
  compress_mb_s: number;
  extract_mb_s: number;
  peak_memory_mb: number;
  cpu_utilization_percent: number;
  verified_lossless: boolean;
}

export interface ThreadScalingPoint {
  threads: number;
  compress_mb_s: number;
  extract_mb_s: number;
  duration_ms: number;
  speedup_factor: number;
}

export interface BenchmarkSuiteReport {
  timestamp_utc: string;
  engine_version: string;
  hardware: HardwareProfile;
  seven_zip_detected: string | null;
  winrar_detected: string | null;
  results: BenchmarkEntryResult[];
  thread_scaling: ThreadScalingPoint[];
  markdown_report_path: string;
}