import { useState, useEffect } from "react"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import { CpuIcon, DownloadIcon, CheckCircleIcon, FolderOpenIcon, ZapIcon } from "lucide-react"
import { useAppContext } from "@/lib/app-context"
import type { AppConfig, ModelInfo, DownloadProgress } from "@/types"

export function ModelSettingsPage() {
  const { t } = useAppContext()
  const [config, setConfig] = useState<AppConfig | null>(null)
  const [models, setModels] = useState<ModelInfo[]>([])
  const [downloading, setDownloading] = useState<string | null>(null)
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null)
  const [downloadError, setDownloadError] = useState<string | null>(null)
  const [activeModel, setActiveModel] = useState<string>("")
  const [switching, setSwitching] = useState(false)

  const loadConfig = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      const cfg = await invoke<AppConfig>("get_app_config")
      setConfig(cfg)
    } catch (err) {
      console.error("Failed to load config:", err)
    }
  }

  const loadModels = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      const mods = await invoke<ModelInfo[]>("list_models")
      setModels(mods)
    } catch (err) {
      console.error("Failed to load models:", err)
    }
  }

  const loadActiveModel = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      const active = await invoke<string>("get_active_model")
      setActiveModel(active)
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
        const { listen } = await import("@tauri-apps/api/event")
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
      const { invoke } = await import("@tauri-apps/api/core")
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
      const { invoke } = await import("@tauri-apps/api/core")
      await invoke<string>("download_specific_model", { modelName })
    } catch (err) {
      setDownloadError(String(err))
      setDownloading(null)
    }
  }

  const handleSwitchModel = async (modelName: string) => {
    setSwitching(true)
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      // This saves config and restarts the app
      await invoke<string>("set_active_model", { modelName })
    } catch (err) {
      console.error("Failed to switch model:", err)
      setSwitching(false)
    }
    // No finally: app will restart, switching stays true
  }

  const modelDescriptions: Record<string, string> = {
    "sense-voice-small": t("models.senseVoiceDesc"),
    "paraformer": t("models.paraformerDesc"),
    "silero-vad": t("models.sileroVadDesc"),
  }

  // ASR models that can be switched
  const asrModels = ["sense-voice-small", "paraformer"]

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

      {/* Model download & management */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <CpuIcon className="size-5" />
            {t("models.title.management")}
          </CardTitle>
          <CardDescription>
            {t("models.desc.management")}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {models.map((model) => {
            const isInstalled = model.installed
            const desc = modelDescriptions[model.name] || `~${model.size}`
            const isActive = model.name === activeModel
            const isAsrModel = asrModels.includes(model.name)
            const isDownloading = downloading === model.name

            return (
              <div key={model.name} className={`p-4 border rounded-lg ${isActive ? "border-primary/50 bg-primary/5" : ""}`}>
                <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className={`p-2 rounded-full ${isActive ? "bg-primary/20" : isInstalled ? "bg-green-100 dark:bg-green-900" : "bg-muted"}`}>
                    {isActive ? (
                      <ZapIcon className="size-5 text-primary" />
                    ) : isInstalled ? (
                      <CheckCircleIcon className="size-5 text-green-600 dark:text-green-400" />
                    ) : (
                      <CpuIcon className="size-5 text-muted-foreground" />
                    )}
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <p className="font-medium">{model.displayName}</p>
                      {isActive && (
                        <span className="text-xs px-1.5 py-0.5 rounded bg-primary/10 text-primary font-medium">
                          {t("models.activeModel")}
                        </span>
                      )}
                    </div>
                    <p className="text-sm text-muted-foreground">{desc}</p>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {isInstalled ? (
                    isAsrModel && !isActive ? (
                      <Button
                        onClick={() => handleSwitchModel(model.name)}
                        disabled={switching}
                        variant="outline"
                        size="sm"
                      >
                        {switching ? t("models.switching") : t("models.switchModel")}
                      </Button>
                    ) : (
                      <span className="text-sm text-green-600 dark:text-green-400 font-medium">
                        {t("models.installed")}
                      </span>
                    )
                  ) : (
                    <Button
                      onClick={() => handleDownload(model.name)}
                      disabled={downloading !== null}
                      size="sm"
                    >
                      <DownloadIcon className="size-4 mr-1" />
                      {isDownloading ? t("models.downloading") : t("models.download")}
                    </Button>
                  )}
                </div>
                </div>
                {model.name === "paraformer" && !isInstalled && (
                  <div className="mt-2 text-xs text-muted-foreground border-t pt-2">
                    <a
                      href="https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-paraformer-trilingual-zh-cantonese-en.tar.bz2"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="underline hover:text-foreground"
                    >
                      {t("models.manualDownload")}
                    </a>
                    <span className="mx-1">·</span>
                    <span>{t("models.manualHint")}</span>
                  </div>
                )}
              </div>
            )
          })}

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
          {!models.some((m) => m.installed && asrModels.includes(m.name)) && !downloading && (
            <div className="text-sm text-muted-foreground py-2">
              {t("models.downloadHint")}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
