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