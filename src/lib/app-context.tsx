import { createContext, useContext, useState, useEffect, type ReactNode } from "react"

export type Language = "zh" | "en"

// Comprehensive i18n dictionary
const dict = {
  zh: {
    // Sidebar
    features: "功能",
    transcribe: "转录",
    history: "历史记录",
    settingsLabel: "设置",
    settings: "设置",
    models: "模型管理",
    about: "关于",

    // SiteHeader
    "header.transcribe": "转录",
    "header.history": "历史记录",
    "header.settings": "设置",
    "header.model-settings": "模型管理",
    "header.models": "模型管理",
    "header.about": "关于",

    // Transcribe page
    "transcribe.title": "转录",
    "transcribe.desc": "支持 MP3, WAV, FLAC, OGG, AAC, M4A, AIFF, MP4, MOV, MKV, WebM 等格式",
    "transcribe.dropFiles": "拖入文件开始转录",
    "transcribe.clickOrDrag": "点击选择或拖拽文件到此处",
    "transcribe.selectFile": "选择文件",
    "transcribe.processing": "处理中...",
    "transcribe.preparing": "准备中...",
    "transcribe.completed": "已完成",
    "transcribe.batch": "批量: {current}/{total} 文件...",
    "transcribe.batchResults": "批量结果 ({succeeded} 成功, {failed} 失败):",
    "transcribe.batchSucceeded": "{count} 文件已完成",
    "transcribe.batchSummary": "{succeeded} 成功, {failed} 失败，耗时 {time}s",
    "transcribe.batchDone": "完成 {count} 个文件，耗时 {time}s",
    "transcribe.segments": "{count} 个分段 · {chars} 字符",
    "transcribe.noContent": "（无内容）",
    "transcribe.speed": "速度",
    "transcribe.realtime": "实时",
    "transcribe.play": "播放",
    "transcribe.pause": "暂停",
    "transcribe.copy": "复制",
    "transcribe.export": "导出",
    "transcribe.new": "新建",
    "transcribe.clear": "清除",
    "transcribe.failed": "转录失败",
    "transcribe.noFile": "没有找到文件",
    "transcribe.emptyExport": "没有可导出的内容",
    "transcribe.tryAgain": "重试",
    "transcribe.newFile": "新文件",
    "transcribe.duration": "时长",
    "transcribe.completedTime": "完成耗时 {time}s",
    "transcribe.speedLabel": "{speed}x 实时",
    "transcribe.loadingModels": "正在加载模型...",
    "transcribe.initFailed": "模型加载失败",
    "transcribe.copyTimed": "复制带时间戳",
    "transcribe.copyText": "复制文本",
    "transcribe.saveSegment": "保存分段",
    "transcribe.cancel": "取消",
    "transcribe.cancelled": "已取消",
    "transcribe.vadSettings": "VAD 设置",
    "transcribe.vadThreshold": "阈值",
    "transcribe.vadMinSilence": "最小静音时长 (s)",
    "transcribe.vadMinSpeech": "最小语音时长 (s)",
    "transcribe.vadMaxSpeech": "最大语音时长 (s)",
    "transcribe.vadThreads": "识别线程数",
    "transcribe.vadApply": "应用",
    "transcribe.rtf": "RTF",
    "transcribe.settingsReloading": "重新加载模型...",
    "transcribe.decoding": "正在解码...",
    "transcribe.done": "完成",
    "transcribe.segmentsWithCount": "{count} 个分段",
    "transcribe.completedToast": "转录完成 · 音频 {duration}s · 耗时 {elapsed}s",

    // History page
    "history.title": "转录历史",
    "history.desc": "查看和管理转录历史记录",
    "history.noRecords": "暂无记录",
    "history.noRecordsHint": "完成转录后将在此显示",
    "history.clearAll": "清空全部",
    "history.segments": "{count} 个分段",

    // Settings page
    "settings.title": "应用设置",
    "settings.desc": "配置转录和导出选项",
    "settings.defaultLang": "默认语言",
    "settings.exportFormat": "默认导出格式",
    "settings.vad": "启用语音活动检测 (VAD)",
    "settings.ffmpegPath": "FFmpeg 路径 (可选)",
    "settings.ffmpegPlaceholder": "留空使用系统 PATH 中的 FFmpeg",
    "settings.ffmpegFound": "已检测",
    "settings.ffmpegNotFound": "未找到",
    "settings.ffmpegVersion": "检测到: {version}",
    "settings.ffmpegMissing": "FFmpeg 未在系统 PATH 中找到，请安装或手动设置路径",
    "settings.ffmpegDownload": "下载 FFmpeg",
    "settings.ffmpegDownloading": "正在下载 FFmpeg...",
    "settings.ffmpegExtracting": "正在解压...",
    "settings.ffmpegInstalled": "FFmpeg 安装完成！",
    "settings.save": "保存设置",
    "settings.saved": "已保存",
    "settings.loading": "加载中...",
    "settings.txtFormat": "纯文本 (.txt)",
    "settings.srtFormat": "SRT 字幕 (.srt)",
    "settings.vttFormat": "VTT 字幕 (.vtt)",

    // Model settings page
    "models.title.storage": "模型存储路径",
    "models.desc.storage": "设置语音识别模型的存放目录",
    "models.save": "保存",
    "models.title.management": "模型管理",
    "models.desc.management": "下载和管理语音识别模型。模型将从 ModelScope.cn 下载。",
    "models.installed": "已安装",
    "models.download": "一键下载",
    "models.downloading": "下载中...",
    "models.senseVoiceDesc": "支持中文/英文/粤语/日/韩，q8 量化约 230MB",
    "models.paraformerDesc": "中/英/粤语三语语音识别，int8 量化约 233MB",
    "models.sileroVadDesc": "Silero VAD 语音活动检测模型，约 2.7MB",
    "models.downloadHint": "从 ModelScope.cn 下载模型，下载完成后自动安装到模型路径中。",
    "models.activeModel": "当前使用",
    "models.switchModel": "切换并重启",
    "models.switching": "重启中...",
    "models.manualDownload": "手动下载",
    "models.manualHint": "可手动下载模型文件放入 paraformer 目录：model.int8.onnx + tokens.txt",

    // About page
    "about.title": "关于",
    "about.desc": "音频/视频转文字转录工具",
    "about.description": "VelociText 是一款跨平台离线语音识别桌面应用。基于 sherpa-onnx 和 SenseVoice-Small，支持本地转录音频和视频文件，无需联网，保障数据隐私。",
    "about.techStack": "技术栈",
    "about.version": "版本",

    // Progress messages
    "extracting_audio": "正在提取音频...",
    "transcribing_full": "正在识别...",
    "vad_detecting": "正在检测语音段...",
    "segmenting_text": "正在分段文本...",
    "transcribing_done": "转录完成",
    "batch_processing": "批量处理中...",
    "batch_complete": "批量转录完成",
  },
  en: {
    // Sidebar
    features: "Features",
    transcribe: "Transcribe",
    history: "History",
    settingsLabel: "Settings",
    settings: "Settings",
    models: "Models",
    about: "About",

    // SiteHeader
    "header.transcribe": "Transcribe",
    "header.history": "History",
    "header.settings": "Settings",
    "header.model-settings": "Models",
    "header.models": "Models",
    "header.about": "About",

    // Transcribe page
    "transcribe.title": "Transcription",
    "transcribe.desc": "Supports MP3, WAV, FLAC, OGG, AAC, M4A, AIFF, MP4, MOV, MKV, WebM and more",
    "transcribe.dropFiles": "Drop files to transcribe",
    "transcribe.clickOrDrag": "Click to select or drag files here",
    "transcribe.selectFile": "Select File",
    "transcribe.processing": "Processing...",
    "transcribe.preparing": "Preparing...",
    "transcribe.completed": "Completed",
    "transcribe.batch": "Batch: {current}/{total} files...",
    "transcribe.batchResults": "Batch Results ({succeeded} succeeded, {failed} failed):",
    "transcribe.batchSucceeded": "{count} file(s) done",
    "transcribe.batchSummary": "{succeeded} succeeded, {failed} failed in {time}s",
    "transcribe.batchDone": "Completed {count} files in {time}s",
    "transcribe.segments": "{count} segments · {chars} chars",
    "transcribe.noContent": "(No content)",
    "transcribe.speed": "Speed",
    "transcribe.realtime": "realtime",
    "transcribe.play": "Play",
    "transcribe.pause": "Pause",
    "transcribe.copy": "Copy",
    "transcribe.export": "Export",
    "transcribe.new": "New",
    "transcribe.clear": "Clear",
    "transcribe.failed": "Transcription Failed",
    "transcribe.noFile": "No file selected",
    "transcribe.emptyExport": "Nothing to export",
    "transcribe.tryAgain": "Try Again",
    "transcribe.newFile": "New File",
    "transcribe.duration": "Duration",
    "transcribe.completedTime": "Completed in {time}s",
    "transcribe.speedLabel": "{speed}x realtime",
    "transcribe.loadingModels": "Loading models...",
    "transcribe.initFailed": "Model initialization failed",
    "transcribe.copyTimed": "Copy with Timestamps",
    "transcribe.copyText": "Copy Text",
    "transcribe.saveSegment": "Save Segment",
    "transcribe.cancel": "Cancel",
    "transcribe.cancelled": "Cancelled",
    "transcribe.vadSettings": "VAD Settings",
    "transcribe.vadThreshold": "Threshold",
    "transcribe.vadMinSilence": "Min Silence (s)",
    "transcribe.vadMinSpeech": "Min Speech (s)",
    "transcribe.vadMaxSpeech": "Max Speech (s)",
    "transcribe.vadThreads": "Recognizer Threads",
    "transcribe.vadApply": "Apply",
    "transcribe.rtf": "RTF",
    "transcribe.settingsReloading": "Reloading models...",
    "transcribe.decoding": "Decoding audio...",
    "transcribe.done": "Done",
    "transcribe.segmentsWithCount": "{count} segment(s)",
    "transcribe.completedToast": "Transcription complete · Audio {duration}s · Elapsed {elapsed}s",

    // History page
    "history.title": "Transcription History",
    "history.desc": "View and manage your transcription history",
    "history.noRecords": "No records yet",
    "history.noRecordsHint": "Completed transcriptions will appear here",
    "history.clearAll": "Clear All",
    "history.segments": "{count} segments",

    // Settings page
    "settings.title": "Settings",
    "settings.desc": "Configure transcription and export options",
    "settings.defaultLang": "Default Language",
    "settings.exportFormat": "Default Export Format",
    "settings.vad": "Enable Voice Activity Detection (VAD)",
    "settings.ffmpegPath": "FFmpeg Path (optional)",
    "settings.ffmpegPlaceholder": "Leave empty to use FFmpeg from system PATH",
    "settings.ffmpegFound": "Detected",
    "settings.ffmpegNotFound": "Not Found",
    "settings.ffmpegVersion": "Detected: {version}",
    "settings.ffmpegMissing": "FFmpeg not found in system PATH. Please install or set path manually.",
    "settings.ffmpegDownload": "Download FFmpeg",
    "settings.ffmpegDownloading": "Downloading FFmpeg...",
    "settings.ffmpegExtracting": "Extracting...",
    "settings.ffmpegInstalled": "FFmpeg installed!",
    "settings.save": "Save Settings",
    "settings.saved": "Saved",
    "settings.loading": "Loading...",
    "settings.txtFormat": "Plain Text (.txt)",
    "settings.srtFormat": "SRT Subtitles (.srt)",
    "settings.vttFormat": "VTT Subtitles (.vtt)",

    // Model settings page
    "models.title.storage": "Model Storage Path",
    "models.desc.storage": "Set the directory for speech recognition models",
    "models.save": "Save",
    "models.title.management": "Model Management",
    "models.desc.management": "Download and manage speech recognition models. Models are downloaded from ModelScope.cn.",
    "models.installed": "Installed",
    "models.download": "Download",
    "models.downloading": "Downloading...",
    "models.senseVoiceDesc": "Supports Chinese/English/Cantonese/Japanese/Korean, q8 quantized ~230MB",
    "models.paraformerDesc": "Trilingual Chinese/English/Cantonese ASR, int8 quantized ~233MB",
    "models.sileroVadDesc": "Silero VAD voice activity detection model, ~2.7MB",
    "models.downloadHint": "Download models from ModelScope.cn. Automatically installed to the model path after download.",
    "models.activeModel": "Active",
    "models.switchModel": "Switch & Restart",
    "models.switching": "Restarting...",
    "models.manualDownload": "Manual Download",
    "models.manualHint": "Or place model.int8.onnx + tokens.txt manually in the paraformer/ directory",

    // About page
    "about.title": "About",
    "about.desc": "Audio/Video to Text Transcription",
    "about.description": "VelociText is a cross-platform desktop application for offline speech recognition. Powered by sherpa-onnx and SenseVoice-Small, it transcribes audio and video files locally without requiring an internet connection, ensuring your data privacy.",
    "about.techStack": "Tech Stack",
    "about.version": "Version",

    // Progress messages
    "extracting_audio": "Extracting audio...",
    "transcribing_full": "Recognizing...",
    "vad_detecting": "Detecting speech segments...",
    "segmenting_text": "Segmenting text...",
    "transcribing_done": "Transcription complete",
    "batch_processing": "Processing batch...",
    "batch_complete": "Batch transcription complete",
  },
}

export function t(language: Language, key: string, vars?: Record<string, string | number>): string {
  const langDict = dict[language] as Record<string, string>
  const fallbackDict = dict["en"] as Record<string, string>
  let text = langDict[key] ?? fallbackDict[key] ?? key
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      text = text.replace(`{${k}}`, String(v))
    }
  }
  return text
}

interface AppContextType {
  theme: "light" | "dark"
  toggleTheme: () => void
  language: Language
  setLanguage: (lang: Language) => void
  t: (key: string, vars?: Record<string, string | number>) => string
}

const AppContext = createContext<AppContextType>({
  theme: "dark",
  toggleTheme: () => {},
  language: "zh",
  setLanguage: () => {},
  t: (key) => key,
})

export function useAppContext() {
  return useContext(AppContext)
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<"light" | "dark">(() => {
    if (typeof window !== "undefined") {
      return (localStorage.getItem("velocitext-theme") as "light" | "dark") || "dark"
    }
    return "dark"
  })

  const [language, setLanguage] = useState<Language>(() => {
    if (typeof window !== "undefined") {
      return (localStorage.getItem("velocitext-lang") as Language) || "zh"
    }
    return "zh"
  })

  useEffect(() => {
    const root = document.documentElement
    if (theme === "dark") {
      root.classList.add("dark")
    } else {
      root.classList.remove("dark")
    }
    localStorage.setItem("velocitext-theme", theme)
  }, [theme])

  useEffect(() => {
    localStorage.setItem("velocitext-lang", language)
  }, [language])

  const toggleTheme = () => {
    setTheme((prev) => (prev === "dark" ? "light" : "dark"))
  }

  const translate = (key: string, vars?: Record<string, string | number>) => t(language, key, vars)

  return (
    <AppContext.Provider value={{ theme, toggleTheme, language, setLanguage, t: translate }}>
      {children}
    </AppContext.Provider>
  )
}