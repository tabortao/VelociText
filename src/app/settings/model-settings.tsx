import { useState, useEffect } from "react"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import { Badge } from "@/components/ui/badge"
import { CpuIcon, DownloadIcon, FolderOpenIcon, ZapIcon } from "lucide-react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useAppContext } from "@/lib/app-context"
import type { AppConfig, ModelInfo, DownloadProgress } from "@/types"

const ASR_MODELS = ["sense-voice-small", "paraformer", "qwen3-asr"] as const

export function ModelSettingsPage() {
  const { t } = useAppContext()
  const [config, setConfig] = useState<AppConfig | null>(null)
  const [models, setModels] = useState<ModelInfo[]>([])
  const [downloading, setDownloading] = useState<string | null>(null)
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null)
  const [downloadError, setDownloadError] = useState<string | null>(null)
  const [activeModel, setActiveModel] = useState<string>("")
  const [switching, setSwitching] = useState(false)
  const [selectedAsr, setSelectedAsr] = useState<string>("sense-voice-small")

  const getModel = (name: string) => models.find((m) => m.name === name)
  const isInstalled = (name: string) => getModel(name)?.installed ?? false

  const loadConfig = async () => {
    try {
      const cfg = await invoke<AppConfig>("get_app_config")
      setConfig(cfg)
    } catch (err) {
      console.error("Failed to load config:", err)
    }
  }

  const loadModels = async () => {
    try {
      const mods = await invoke<ModelInfo[]>("list_models")
      setModels(mods)
    } catch (err) {
      console.error("Failed to load models:", err)
    }
  }

  const loadActiveModel = async () => {
    try {
      const active = await invoke<string>("get_active_model")
      setActiveModel(active)
      setSelectedAsr(active)
    } catch {
      // ignore
    }
  }

  useEffect(() => {
    loadConfig()
    loadModels()
    loadActiveModel()

    let unlisten: (() => void) | undefined
    const setupListener = async () => {
      try {
        unlisten = await listen<DownloadProgress>("model-download-progress", (event) => {
          setDownloadProgress(event.payload)
          if (event.payload.stage === "completed") {
            setDownloading(null)
            loadModels()
            loadActiveModel()
          }
        })
      } catch (err) {
        console.error("Failed to listen download events:", err)
      }
    }
    setupListener()

    return () => {
      if (unlisten) unlisten()
    }
  }, [])

  const saveModelPath = async () => {
    if (!config) return
    try {
      await invoke("set_app_config", { newConfig: config })
      loadModels()
    } catch (err) {
      console.error("Failed to save model path:", err)
    }
  }

  const handleDownload = async (modelName: string) => {
    setDownloading(modelName)
    setDownloadError(null)
    setDownloadProgress(null)
    try {
      await invoke<string>("download_specific_model", { modelName })
    } catch (err) {
      setDownloadError(String(err))
      setDownloading(null)
    }
  }

  const handleSwitchModel = async (modelName: string) => {
    setSwitching(true)
    try {
      await invoke<string>("set_active_model", { modelName })
    } catch (err) {
      console.error("Failed to switch model:", err)
      setSwitching(false)
    }
  }

  return (
    <div className="px-4 lg:px-6 space-y-4">
      {/* Model storage path */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <FolderOpenIcon className="size-5" />
            {t("models.title.storage")}
          </CardTitle>
          <CardDescription>{t("models.desc.storage")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex gap-2">
            <Input
              value={config?.modelPath || ""}
              onChange={(e) => setConfig((prev) => prev ? { ...prev, modelPath: e.target.value } : prev)}
              placeholder={t("models.desc.storage")}
              className="flex-1"
            />
            <Button variant="outline" onClick={saveModelPath}>
              {t("models.save")}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* ASR Models */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <CpuIcon className="size-5" />
            {t("models.title.management")}
          </CardTitle>
          <CardDescription>{t("models.desc.management")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {/* ASR Model Selection */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-muted-foreground">ASR</h4>
            <div className="flex items-center gap-3">
              <select
                value={selectedAsr}
                onChange={(e) => setSelectedAsr(e.target.value)}
                className="h-9 rounded-md border border-input bg-background px-3 py-1 text-sm flex-1"
              >
                {ASR_MODELS.map((key) => {
                  const model = getModel(key)
                  return (
                    <option key={key} value={key}>
                      {model?.displayName ?? key}{isInstalled(key) ? " ✓" : " (not installed)"}
                    </option>
                  )
                })}
              </select>

              {(() => {
                const selectedModel = getModel(selectedAsr)
                if (!selectedModel) return null

                if (!selectedModel.installed) {
                  return (
                    <Button
                      onClick={() => handleDownload(selectedAsr)}
                      disabled={downloading !== null}
                      size="sm"
                    >
                      <DownloadIcon className="size-4 mr-1" />
                      {downloading === selectedAsr ? t("models.downloading") : t("models.download")}
                    </Button>
                  )
                }

                if (selectedAsr === activeModel) {
                  return (
                    <Badge variant="default" className="h-8 px-3">
                      <ZapIcon className="size-3 mr-1" />
                      {t("models.activeModel")}
                    </Badge>
                  )
                }

                return (
                  <Button
                    onClick={() => handleSwitchModel(selectedAsr)}
                    disabled={switching}
                    variant="outline"
                    size="sm"
                  >
                    {switching ? t("models.switching") : t("models.switchModel")}
                  </Button>
                )
              })()}
            </div>
            {(() => {
              const sm = getModel(selectedAsr)
              if (sm && sm.installed && sm.path) {
                return (
                  <p className="text-xs text-muted-foreground">
                    {sm.path}
                  </p>
                )
              }
              return null
            })()}
          </div>

          {/* Download progress */}
          {downloading && downloadProgress && (
            <div className="space-y-2 p-4 border rounded-lg bg-muted/30">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">{downloadProgress.stage}</span>
                <span className="text-sm text-muted-foreground">
                  {downloadProgress.percentage.toFixed(0)}%
                </span>
              </div>
              <Progress value={downloadProgress.percentage} className="h-2" />
              {downloadProgress.total > 0 && (
                <p className="text-xs text-muted-foreground">
                  {(downloadProgress.downloaded / 1024 / 1024).toFixed(1)} MB
                  {" / "}
                  {(downloadProgress.total / 1024 / 1024).toFixed(1)} MB
                </p>
              )}
            </div>
          )}

          {/* Download error */}
          {downloadError && (
            <div className="p-3 border border-red-200 rounded-lg bg-red-50 dark:bg-red-950 dark:border-red-800">
              <p className="text-sm text-red-600 dark:text-red-400">{downloadError}</p>
            </div>
          )}

          {/* Hint */}
          {!models.some((m) => m.installed && ASR_MODELS.includes(m.name as typeof ASR_MODELS[number])) && !downloading && (
            <div className="text-sm text-muted-foreground py-2">
              {t("models.downloadHint")}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}