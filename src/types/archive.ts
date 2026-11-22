export type ArchiveFormat = "Zip" | "Kinetic" | "SevenZ" | "Rar";

export type CompressionMode =
  | "Store"
  | "Fast"
  | "Balanced"
  | "Maximum"
  | "Ultra"
  | "Automatic";

export interface HardwareProfile {
  logical_cores: number;
  total_memory_mb: number;
  available_memory_mb: number;
  recommended_threads: number;
  max_safe_memory_arena_mb: number;
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

export interface EngineStatusPayload {
  engine_name: string;
  engine_version: string;
  default_format: ArchiveFormat;
  default_mode: CompressionMode;
  hardware: HardwareProfile;
  supported_formats: FormatCapability[];
  compression_modes: ModeCapability[];
}
