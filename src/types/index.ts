/** 转录结果分段 */
export interface TranscribeSegment {
  start: number;
  end: number;
  text: string;
}

/** 转录完成结果 */
export interface TranscribeResult {
  segments: TranscribeSegment[];
  audioDuration: number;
  elapsedMs: number;
  fileName: string;
  modelType: string;
}

/** 转录进度事件 */
export interface ProgressEvent {
  percent: number;
  stage: string;
  message: string;
}

/** 转录选项 */
export interface TranscribeOptions {
  filePath: string;
  language?: string;
  useVad?: boolean;
  modelType?: string;
  hotwordsFile?: string;
}

/** 转录任务状态 */
export type TaskStatus =
  | "pending"
  | "extractingAudio"
  | "transcribing"
  | "completed"
  | "failed"
  | "cancelled";

/** 转录任务 */
export interface TranscribeTask {
  id: string;
  filePath: string;
  fileName: string;
  status: TaskStatus;
  progress: number;
  segments: TranscribeSegment[];
  createdAt: string;
  completedAt: string | null;
  error: string | null;
}

/** 支持的语言 */
export interface Language {
  code: string;
  name: string;
}

/** 应用配置 */
export interface AppConfig {
  modelPath: string;
  defaultLanguage: string;
  exportFormat: string;
  useVad: boolean;
  ffmpegPath: string | null;
  activeModel: string;
  sidebarCollapsed: boolean;
}

/** 模型下载进度 */
export interface DownloadProgress {
  modelName: string;
  downloaded: number;
  total: number;
  percentage: number;
  stage: string;
}

export interface ModelInfo {
  name: string;
  displayName: string;
  size: string;
  installed: boolean;
  path: string | null;
}

/** 批量转录单个文件结果 */
export interface BatchFileResult {
  fileName: string;
  filePath: string;
  success: boolean;
  segments: TranscribeSegment[];
  audioDuration: number;
  elapsedMs: number;
  modelType: string;
  error: string | null;
}

/** 批量转录总结果 */
export interface BatchResult {
  results: BatchFileResult[];
  totalFiles: number;
  succeeded: number;
  failed: number;
  totalElapsedMs: number;
  totalAudioDuration: number;
}

/** 历史记录条目 */
export interface HistoryEntry {
  id: string;
  fileName: string;
  filePath: string;
  durationSecs: number;
  text: string;
  language: string;
  modelType: string;
  segmentCount: number;
  createdAt: string;
}

// ============================================================================
// 流式转录类型 (对标 sherpa-onnx non-streaming-speech-recognition-from-file)
// ============================================================================

/** 流式转录分段结果 */
export interface StreamingSegment {
  start: number;
  end: number;
  text: string;
}

/** 轮询获取的处理状态 */
export interface ProcessingState {
  percent: number;
  status: string; // "idle" | "processing" | "done" | "cancelled" | "error:..."
  segments: StreamingSegment[];
  elapsedSecs: number;
  audioDurationSecs: number;
}

/** 模型初始化状态 */
export interface InitStatus {
  status: number; // 0=pending, 1=ready, 2=error
  error: string;
  numThreads: number;
}

/** VAD 可调参数 */
export interface VadSettings {
  threshold: number;
  minSilenceDuration: number;
  minSpeechDuration: number;
  maxSpeechDuration: number;
  numThreads: number;
}

// ============================================================================
// 词典类型
// ============================================================================

/** 热词条目 */
export interface HotwordEntry {
  word: string;
  weight: number;
}

/** 替换条目 */
export interface ReplacementEntry {
  original: string;
  replacement: string;
}

/** 词典配置 */
export interface DictionaryConfig {
  hotwords: HotwordEntry[];
  replacements: ReplacementEntry[];
}

// ============================================================================
// OCR 类型
// ============================================================================

/** OCR 文本块 */
export interface OcrTextBlock {
  text: string;
  confidence: number;
  boxPoints: [number, number][]; // 4 个角点坐标
}

/** OCR 识别结果 */
export interface OcrResult {
  textBlocks: OcrTextBlock[];
  totalTimeMs: number;
}