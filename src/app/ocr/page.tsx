import { useState, useEffect, useRef, useCallback } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Separator } from "@/components/ui/separator"
import { Badge } from "@/components/ui/badge"
import {
  UploadIcon,
  ImageIcon,
  CopyIcon,
  Trash2Icon,
  FileTextIcon,
  ScanTextIcon,
  AlertTriangleIcon,
} from "lucide-react"
import { convertFileSrc } from "@tauri-apps/api/core"
import { open } from "@tauri-apps/plugin-dialog"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { useAppContext } from "@/lib/app-context"
import type { OcrResult, ModelInfo } from "@/types"

type OCRState = "idle" | "loading" | "completed" | "error"

const MODEL_NAMES = ["ppocr-v4", "ppocr-v5", "ppocr-v6"] as const
const MODEL_DISPLAY: Record<string, string> = {
  "ppocr-v4": "PaddleOCR V4",
  "ppocr-v5": "PaddleOCR V5",
  "ppocr-v6": "PaddleOCR V6",
}

export function OCRPage() {
  const { t } = useAppContext()
  const [ocrState, setOCRState] = useState<OCRState>("idle")
  const [imagePath, setImagePath] = useState<string | null>(null)
  const [imageUrl, setImageUrl] = useState<string>("")
  const [result, setResult] = useState<OcrResult | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isDragOver, setIsDragOver] = useState(false)
  const [activeModel, setActiveModel] = useState("ppocr-v5")
  const [installedModels, setInstalledModels] = useState<Set<string>>(new Set())
  const [flashMessage, setFlashMessage] = useState("")
  const flashTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const modelInstalled = installedModels.has(activeModel)

  const showFlash = useCallback((msg: string) => {
    setFlashMessage(msg)
    if (flashTimerRef.current) clearTimeout(flashTimerRef.current)
    flashTimerRef.current = setTimeout(() => setFlashMessage(""), 2000)
  }, [])

  useEffect(() => {
    return () => {
      if (flashTimerRef.current) clearTimeout(flashTimerRef.current)
    }
  }, [])

  // Load active OCR model and check which models are installed
  useEffect(() => {
    const load = async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core")
        const active = await invoke<string>("ocr_get_active_model")
        setActiveModel(active)

        // Check which models are installed
        const models = await invoke<ModelInfo[]>("list_models")
        const installed = new Set<string>()
        for (const m of models) {
          if (MODEL_NAMES.includes(m.name as typeof MODEL_NAMES[number]) && m.installed) {
            installed.add(m.name)
          }
        }
        setInstalledModels(installed)
      } catch {
        // ignore
      }
    }
    load()
  }, [])

  // Handle file drop events
  useEffect(() => {
    let unlistenDrop: UnlistenFn | undefined
    let unlistenHover: UnlistenFn | undefined
    let unlistenHoverLeave: UnlistenFn | undefined

    const setup = async () => {
      try {
        unlistenDrop = await listen<string>("tauri://file-drop", (event) => {
          try {
            const paths: string[] = JSON.parse(event.payload)
            if (paths.length > 0) {
              handleFile(paths[0])
            }
          } catch {
            // ignore
          }
        })
        unlistenHover = await listen<boolean>("tauri://file-drop-hover", () => {
          setIsDragOver(true)
        })
        unlistenHoverLeave = await listen<boolean>("tauri://file-drop-hover", (event) => {
          if (!event.payload) setIsDragOver(false)
        })
      } catch {
        // ignore
      }
    }
    setup()

    return () => {
      unlistenDrop?.()
      unlistenHover?.()
      unlistenHoverLeave?.()
    }
  }, [])

  const handleFile = useCallback(async (path: string) => {
    const ext = path.split(".").pop()?.toLowerCase() ?? ""
    const imageExts = ["png", "jpg", "jpeg", "bmp", "webp", "tiff", "tif"]
    if (!imageExts.includes(ext)) {
      setError(t("ocr.desc"))
      return
    }

    setImagePath(path)
    setImageUrl(convertFileSrc(path))
    setResult(null)
    setError(null)
    setOCRState("idle")

    // Auto-start OCR only if model is installed
    if (modelInstalled) {
      startOCR(path)
    }
  }, [modelInstalled, t])

  const handleOpenFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Images",
            extensions: ["png", "jpg", "jpeg", "bmp", "webp", "tiff", "tif"],
          },
        ],
      })
      if (selected) {
        handleFile(selected as string)
      }
    } catch (err) {
      console.error("Failed to open file:", err)
    }
  }

  const startOCR = async (path: string) => {
    if (!modelInstalled) {
      setError(t("ocr.modelNotInstalled", { model: MODEL_DISPLAY[activeModel] ?? activeModel }))
      return
    }

    setOCRState("loading")
    setError(null)

    try {
      const { invoke } = await import("@tauri-apps/api/core")
      const res = await invoke<OcrResult>("ocr_recognize", {
        imagePath: path,
        modelVersion: activeModel,
      })
      setResult(res)
      setOCRState("completed")
    } catch (err) {
      setError(String(err))
      setOCRState("error")
    }
  }

  const handleModelChange = async (model: string) => {
    setActiveModel(model)
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      await invoke("ocr_set_active_model", { modelName: model })

      // Refresh installed status
      const models = await invoke<ModelInfo[]>("list_models")
      const installed = new Set<string>()
      for (const m of models) {
        if (MODEL_NAMES.includes(m.name as typeof MODEL_NAMES[number]) && m.installed) {
          installed.add(m.name)
        }
      }
      setInstalledModels(installed)
    } catch {
      // ignore
    }

    // Re-run OCR only if the new model is installed and image is loaded
    if (imagePath && installedModels.has(model)) {
      setTimeout(() => startOCR(imagePath), 100)
    }
  }

  const handleClear = () => {
    setImagePath(null)
    setImageUrl("")
    setResult(null)
    setError(null)
    setOCRState("idle")
  }

  const handleCopyText = () => {
    if (!result) return
    const text = result.textBlocks.map((b) => b.text).join("\n")
    navigator.clipboard.writeText(text).then(() => {
      showFlash(t("ocr.copyText"))
    })
  }

  const handleExportTxt = async () => {
    if (!result || !imagePath) return
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      const text = result.textBlocks.map((b) => b.text).join("\n")
      const baseName = imagePath.replace(/\.[^.]+$/, "")
      const exportPath = `${baseName}_ocr.txt`
      await invoke("write_text_file", { path: exportPath, content: text })
      await invoke("open_file_with_system", { path: exportPath })
      showFlash(t("ocr.exportTxt"))
    } catch (err) {
      setError(String(err))
    }
  }

  return (
    <div className="px-4 lg:px-6 space-y-4">
      {/* Toolbar */}
      <div className="flex items-center gap-2 flex-wrap">
        <select
          value={activeModel}
          onChange={(e) => handleModelChange(e.target.value)}
          className="h-9 rounded-md border border-input bg-background px-3 py-1 text-sm"
        >
          {MODEL_NAMES.map((key) => (
            <option key={key} value={key}>
              {MODEL_DISPLAY[key]}{installedModels.has(key) ? "" : " (not installed)"}
            </option>
          ))}
        </select>
        <Button onClick={handleOpenFile} variant="outline" size="sm">
          <UploadIcon className="size-4 mr-1" />
          {t("ocr.selectImage")}
        </Button>
        <Button onClick={handleClear} variant="ghost" size="sm" disabled={!imagePath}>
          <Trash2Icon className="size-4 mr-1" />
          {t("ocr.clear")}
        </Button>
        {flashMessage && (
          <Badge variant="secondary" className="ml-2">
            {flashMessage}
          </Badge>
        )}
      </div>

      {/* Model not installed warning */}
      {!modelInstalled && (
        <div className="p-3 border border-amber-200 rounded-lg bg-amber-50 dark:bg-amber-950 dark:border-amber-800 flex items-start gap-2">
          <AlertTriangleIcon className="size-4 mt-0.5 text-amber-600 dark:text-amber-400 shrink-0" />
          <p className="text-sm text-amber-700 dark:text-amber-300">
            {t("ocr.modelNotInstalled", { model: MODEL_DISPLAY[activeModel] ?? activeModel })}
          </p>
        </div>
      )}

      {error && (
        <div className="p-3 border border-red-200 rounded-lg bg-red-50 dark:bg-red-950 dark:border-red-800">
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
        </div>
      )}

      {!imagePath ? (
        /* Idle state: drag-and-drop zone */
        <Card
          className={`border-2 border-dashed transition-colors ${
            isDragOver ? "border-primary bg-primary/5" : "border-muted-foreground/25"
          }`}
        >
          <CardContent className="flex flex-col items-center justify-center py-16 gap-4">
            <ImageIcon className="size-16 text-muted-foreground" />
            <div className="text-center space-y-2">
              <p className="text-lg font-medium">{t("ocr.dropImages")}</p>
              <p className="text-sm text-muted-foreground">{t("ocr.clickOrDrag")}</p>
              <p className="text-xs text-muted-foreground">{t("ocr.desc")}</p>
            </div>
            <Button onClick={handleOpenFile} variant="default">
              <UploadIcon className="size-4 mr-2" />
              {t("ocr.selectImage")}
            </Button>
          </CardContent>
        </Card>
      ) : (
        /* Image loaded: show preview + results */
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {/* Image preview */}
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm flex items-center gap-2">
                <ImageIcon className="size-4" />
                {imagePath.split(/[/\\]/).pop()}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="relative rounded-lg overflow-hidden bg-muted">
                <img
                  src={imageUrl}
                  alt="Preview"
                  className="max-w-full max-h-[500px] object-contain mx-auto"
                />
              </div>
            </CardContent>
          </Card>

          {/* Results panel */}
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm flex items-center gap-2">
                <ScanTextIcon className="size-4" />
                {ocrState === "loading"
                  ? t("ocr.processing")
                  : ocrState === "completed"
                    ? t("ocr.completed")
                    : t("ocr.title")}
              </CardTitle>
              {result && (
                <CardDescription>
                  {t("ocr.textBlocks", { count: result.textBlocks.length })}
                </CardDescription>
              )}
            </CardHeader>
            <CardContent>
              {ocrState === "loading" && (
                <div className="flex items-center justify-center py-12">
                  <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                </div>
              )}

              {ocrState === "completed" && result && (
                <div className="space-y-3">
                  {result.textBlocks.length === 0 ? (
                    <p className="text-sm text-muted-foreground text-center py-8">
                      {t("ocr.noTextFound")}
                    </p>
                  ) : (
                    <div className="space-y-2 max-h-[450px] overflow-y-auto">
                      {result.textBlocks.map((block, idx) => (
                        <div
                          key={idx}
                          className="p-3 rounded-lg border bg-muted/30 hover:bg-muted/50 transition-colors"
                        >
                          <div className="flex items-start justify-between gap-2">
                            <p className="text-sm font-medium leading-relaxed break-all">
                              {block.text}
                            </p>
                            <Badge variant="secondary" className="shrink-0 text-xs">
                              {(block.confidence * 100).toFixed(1)}%
                            </Badge>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}

                  <Separator />

                  <div className="flex gap-2">
                    <Button
                      onClick={handleCopyText}
                      variant="outline"
                      size="sm"
                      disabled={result.textBlocks.length === 0}
                    >
                      <CopyIcon className="size-4 mr-1" />
                      {t("ocr.copyText")}
                    </Button>
                    <Button
                      onClick={handleExportTxt}
                      variant="outline"
                      size="sm"
                      disabled={result.textBlocks.length === 0}
                    >
                      <FileTextIcon className="size-4 mr-1" />
                      {t("ocr.exportTxt")}
                    </Button>
                  </div>
                </div>
              )}

              {ocrState === "idle" && !modelInstalled && (
                <div className="text-center py-8">
                  <p className="text-sm text-muted-foreground">
                    {t("ocr.noModel")}
                  </p>
                </div>
              )}

              {ocrState === "idle" && modelInstalled && (
                <div className="text-center py-8">
                  <p className="text-sm text-muted-foreground">
                    {t("ocr.clickOrDrag")}
                  </p>
                  <Button
                    onClick={() => imagePath && startOCR(imagePath)}
                    variant="default"
                    size="sm"
                    className="mt-3"
                  >
                    <ScanTextIcon className="size-4 mr-1" />
                    {t("ocr.title")}
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  )
}