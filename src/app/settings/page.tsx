import { useState, useEffect } from "react"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Button } from "@/components/ui/button"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Checkbox } from "@/components/ui/checkbox"
import { Badge } from "@/components/ui/badge"
import { Progress } from "@/components/ui/progress"
import { Settings2Icon, SaveIcon, CheckCircleIcon, XCircleIcon, DownloadIcon } from "lucide-react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
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

  const loadConfig = async () => {
    try {
      const cfg = await invoke<AppConfig>("get_app_config")
      setConfig(cfg)
    } catch (err) {
      console.error("Failed to load config:", err)
    }
  }

  const loadLanguages = async () => {
    try {
      const langs = await invoke<Language[]>("get_languages")
      setLanguages(langs)
    } catch (err) {
      console.error("Failed to load languages:", err)
    }
  }

  const checkFfmpeg = async () => {
    try {
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

          <Button onClick={handleSave} className="w-full">
            <SaveIcon className="size-4 mr-2" />
            {saved ? t("settings.saved") : t("settings.save")}
          </Button>
        </CardContent>
      </Card>
    </div>
  )
}