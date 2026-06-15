import { useState, useEffect, useRef } from "react"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Button } from "@/components/ui/button"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Checkbox } from "@/components/ui/checkbox"
import { Badge } from "@/components/ui/badge"
import { Progress } from "@/components/ui/progress"
import { Settings2Icon, SaveIcon, CheckCircleIcon, XCircleIcon, DownloadIcon, AlertTriangleIcon } from "lucide-react"
import { useAppContext } from "@/lib/app-context"
import type { AppConfig, Language } from "@/types"

interface FfmpegProgress {
  stage: string
  percentage: number
  message: string
}

export function SettingsPage() {
  const { t } = useAppContext()
  const [config, setConfig] = useState<AppConfig | null>(null)
  const [languages, setLanguages] = useState<Language[]>([])
  const [saved, setSaved] = useState(false)
  const [ffmpegStatus, setFfmpegStatus] = useState<{ found: boolean; version: string } | null>(null)
  const [ffmpegDownloading, setFfmpegDownloading] = useState(false)
  const [ffmpegProgress, setFfmpegProgress] = useState<FfmpegProgress | null>(null)
  const [recordingShortcut, setRecordingShortcut] = useState(false)
  const [shortcutConflict, setShortcutConflict] = useState("")
  const recordingRef = useRef(false)

  // Known reserved shortcuts that may conflict
  const RESERVED_SHORTCUTS: Record<string, string> = {
    "Ctrl+C": "系统复制",
    "Ctrl+V": "系统粘贴",
    "Ctrl+X": "系统剪切",
    "Ctrl+Z": "系统撤销",
    "Ctrl+Y": "系统重做",
    "Ctrl+A": "全选",
    "Ctrl+S": "保存",
    "Ctrl+W": "关闭标签",
    "Ctrl+T": "新建标签",
    "Ctrl+N": "新建窗口",
    "Ctrl+P": "打印",
    "Ctrl+F": "查找",
    "Ctrl+H": "替换",
    "Ctrl+R": "刷新",
    "Ctrl+Shift+Esc": "任务管理器",
    "Alt+Tab": "切换窗口",
    "Alt+F4": "关闭窗口",
    "Win+D": "显示桌面",
    "Win+E": "文件资源管理器",
    "Win+R": "运行",
    "Win+L": "锁定屏幕",
  }

  const formatShortcut = (e: KeyboardEvent): string => {
    const parts: string[] = []
    if (e.ctrlKey) parts.push("Ctrl")
    if (e.altKey) parts.push("Alt")
    if (e.shiftKey) parts.push("Shift")
    if (e.metaKey) parts.push("Meta")
    // Only add the key if it's not a modifier
    const key = e.key
    if (!["Control", "Alt", "Shift", "Meta"].includes(key)) {
      if (key === " ") parts.push("Space")
      else if (key.length === 1) parts.push(key.toUpperCase())
      else parts.push(key)
    }
    return parts.join("+")
  }

  const checkShortcutConflict = (shortcut: string): string => {
    if (RESERVED_SHORTCUTS[shortcut]) {
      return `与 ${RESERVED_SHORTCUTS[shortcut]} 冲突`
    }
    return ""
  }

  const handleStartRecording = () => {
    setRecordingShortcut(true)
    setShortcutConflict("")
    recordingRef.current = true
  }

  useEffect(() => {
    if (!recordingShortcut) return

    const handler = (e: KeyboardEvent) => {
      if (!recordingRef.current) return
      // Ignore pure modifier key presses
      if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return
      e.preventDefault()
      e.stopPropagation()

      const shortcut = formatShortcut(e)
      const conflict = checkShortcutConflict(shortcut)
      setShortcutConflict(conflict)
      setConfig((prev) => (prev ? { ...prev, ocrScreenshotShortcut: shortcut } : prev))
      setRecordingShortcut(false)
      recordingRef.current = false
    }

    window.addEventListener("keydown", handler, true)
    return () => window.removeEventListener("keydown", handler, true)
  }, [recordingShortcut])

  const loadConfig = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      const cfg = await invoke<AppConfig>("get_app_config")
      setConfig(cfg)
    } catch (err) {
      console.error("Failed to load config:", err)
    }
  }

  const loadLanguages = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      const langs = await invoke<Language[]>("get_languages")
      setLanguages(langs)
    } catch (err) {
      console.error("Failed to load languages:", err)
    }
  }

  const checkFfmpeg = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      const version = await invoke<string>("check_ffmpeg")
      setFfmpegStatus({ found: true, version })
    } catch (err) {
      setFfmpegStatus({ found: false, version: String(err) })
    }
  }

  useEffect(() => {
    loadConfig()
    loadLanguages()
    checkFfmpeg()

    let unlisten: (() => void) | undefined
    const setupListener = async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event")
        unlisten = await listen<FfmpegProgress>("ffmpeg-download-progress", (event) => {
          setFfmpegProgress(event.payload)
          if (event.payload.stage === "completed") {
            setFfmpegDownloading(false)
            setFfmpegProgress(null)
            checkFfmpeg()
            loadConfig()
          } else if (event.payload.stage === "error") {
            setFfmpegDownloading(false)
            setFfmpegProgress(null)
          }
        })
      } catch (err) {
        console.error("Failed to listen ffmpeg events:", err)
      }
    }
    setupListener()

    return () => {
      if (unlisten) unlisten()
    }
  }, [])

  const handleSave = async () => {
    if (!config) return
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      await invoke("set_app_config", { newConfig: config })
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } catch (err) {
      console.error("Failed to save config:", err)
    }
  }

  const handleDownloadFfmpeg = async () => {
    setFfmpegDownloading(true)
    setFfmpegProgress(null)
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      await invoke<string>("download_ffmpeg")
    } catch (err) {
      console.error("FFmpeg download failed:", err)
      setFfmpegDownloading(false)
    }
  }

  if (!config) {
    return (
      <div className="px-4 lg:px-6 space-y-4">
        <div className="flex items-center justify-center py-16">
          <p className="text-muted-foreground">{t("settings.loading")}</p>
        </div>
      </div>
    )
  }

  return (
    <div className="px-4 lg:px-6 space-y-4">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Settings2Icon className="size-5" />
            {t("settings.title")}
          </CardTitle>
          <CardDescription>{t("settings.desc")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          {/* Default Language */}
          <div className="space-y-2">
            <Label>{t("settings.defaultLang")}</Label>
            <Select
              value={config.defaultLanguage}
              onValueChange={(v) => setConfig({ ...config, defaultLanguage: v })}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {languages.map((lang) => (
                  <SelectItem key={lang.code} value={lang.code}>
                    {lang.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* Export Format */}
          <div className="space-y-2">
            <Label>{t("settings.exportFormat")}</Label>
            <Select
              value={config.exportFormat}
              onValueChange={(v) => setConfig({ ...config, exportFormat: v })}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="txt">{t("settings.txtFormat")}</SelectItem>
                <SelectItem value="srt">{t("settings.srtFormat")}</SelectItem>
                <SelectItem value="vtt">{t("settings.vttFormat")}</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* VAD */}
          <div className="flex items-center gap-2">
            <Checkbox
              id="use-vad"
              checked={config.useVad}
              onCheckedChange={(checked) =>
                setConfig({ ...config, useVad: !!checked })
              }
            />
            <Label htmlFor="use-vad">{t("settings.vad")}</Label>
          </div>

          {/* FFmpeg path */}
          <div className="space-y-2">
            <Label htmlFor="ffmpeg-path">{t("settings.ffmpegPath")}</Label>
            <div className="flex items-center gap-2">
              <Input
                id="ffmpeg-path"
                value={config.ffmpegPath || ""}
                onChange={(e) => setConfig({ ...config, ffmpegPath: e.target.value || null })}
                placeholder={t("settings.ffmpegPlaceholder")}
                className="flex-1"
              />
              {ffmpegStatus && (
                <Badge
                  variant={ffmpegStatus.found ? "default" : "destructive"}
                  className="shrink-0 gap-1"
                >
                  {ffmpegStatus.found ? (
                    <CheckCircleIcon className="size-3" />
                  ) : (
                    <XCircleIcon className="size-3" />
                  )}
                  {ffmpegStatus.found ? t("settings.ffmpegFound") : t("settings.ffmpegNotFound")}
                </Badge>
              )}
            </div>
            {ffmpegStatus && ffmpegStatus.found && (
              <p className="text-xs text-muted-foreground">
                {t("settings.ffmpegVersion", { version: ffmpegStatus.version })}
              </p>
            )}
            {ffmpegStatus && !ffmpegStatus.found && !ffmpegDownloading && (
              <div className="space-y-2">
                <p className="text-xs text-red-500">
                  {t("settings.ffmpegMissing")}
                </p>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleDownloadFfmpeg}
                >
                  <DownloadIcon className="size-3.5 mr-1" />
                  {t("settings.ffmpegDownload")}
                </Button>
              </div>
            )}

            {/* FFmpeg download progress */}
            {ffmpegDownloading && ffmpegProgress && (
              <div className="space-y-2 p-3 border rounded-lg bg-muted/30">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium">
                    {ffmpegProgress.stage === "downloading"
                      ? t("settings.ffmpegDownloading")
                      : ffmpegProgress.stage === "extracting"
                        ? t("settings.ffmpegExtracting")
                        : ffmpegProgress.message}
                  </span>
                  <span className="text-sm text-muted-foreground">
                    {ffmpegProgress.percentage.toFixed(0)}%
                  </span>
                </div>
                <Progress value={ffmpegProgress.percentage} className="h-2" />
                {ffmpegProgress.stage === "downloading" && (
                  <p className="text-xs text-muted-foreground">{ffmpegProgress.message}</p>
                )}
              </div>
            )}
          </div>

          {/* OCR Screenshot Shortcut */}
          <div className="space-y-2">
            <Label>{t("settings.ocrScreenshotShortcut")}</Label>
            <div className="flex items-center gap-2">
              <div
                className={`flex-1 h-9 rounded-md border px-3 py-1 text-sm flex items-center cursor-pointer select-none ${
                  recordingShortcut
                    ? "border-primary ring-2 ring-primary/20 bg-primary/5"
                    : "border-input bg-background hover:border-muted-foreground/30"
                }`}
                onClick={handleStartRecording}
                title="点击后按下新的快捷键组合"
              >
                {recordingShortcut ? (
                  <span className="text-muted-foreground animate-pulse">按下快捷键...</span>
                ) : (
                  <span className="font-mono">{config.ocrScreenshotShortcut || "Ctrl+Shift+O"}</span>
                )}
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={handleStartRecording}
                disabled={recordingShortcut}
              >
                修改
              </Button>
            </div>
            {shortcutConflict && (
              <div className="flex items-center gap-1.5 text-amber-600 dark:text-amber-400">
                <AlertTriangleIcon className="size-3.5" />
                <p className="text-xs">{shortcutConflict}</p>
              </div>
            )}
            {!shortcutConflict && !recordingShortcut && (
              <p className="text-xs text-muted-foreground">
                {t("settings.ocrScreenshotShortcutDesc")}
              </p>
            )}
          </div>

          <Button onClick={handleSave} className="w-full">
            <SaveIcon className="size-4 mr-2" />
            {saved ? t("settings.saved") : t("settings.save")}
          </Button>
        </CardContent>
      </Card>
    </div>
  )
}