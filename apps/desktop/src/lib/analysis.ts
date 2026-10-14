/** TypeScript mirror of `sentinel_core::analysis`. Rust is the source of truth. */

export type AnalysisStage = "walking" | "duplicates";

export interface AnalysisProgress {
  stage: AnalysisStage;
  files: number;
  bytes: number;
}

export interface FolderUsage {
  path: string;
  sizeBytes: number;
}

export interface LargeFile {
  path: string;
  sizeBytes: number;
}

export interface DuplicateGroup {
  sizeBytes: number;
  paths: string[];
}

export interface HomeAnalysis {
  folders: FolderUsage[];
  largestFiles: LargeFile[];
  duplicates: DuplicateGroup[];
  totalFiles: number;
  totalBytes: number;
  truncated: boolean;
}
