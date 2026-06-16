import { useState, useEffect, useRef, useCallback } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
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
  CameraIcon,
  CheckCircleIcon,
  XCircleIcon,
  ChevronDownIcon,
  ChevronRightIcon,
} from "lucide-react"
import { invoke, convertFileSrc } from "@tauri-apps/api/core"
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

const IMAGE_EXTS = ["png", "jpg", "jpeg", "bmp", "webp", "tiff", "tif"]

interface BatchOcrItem {
  path: string
  fileName: string
  imageUrl: string
  state: OCRState
  result: OcrResult | null
  error: string | null
}

interface OCRPageProps {
  onScreenshotTrigger?: () => void
}

export function OCRPage({ onScreenshotTrigger }: OCRPageProps) {
  const { t } = useAppContext()
  const [items, setItems] = useState<BatchOcrItem[]>([])
  const [isDragOver, setIsDragOver] = useState(false)
  const [activeModel, setActiveModel] = useState("ppocr-v5")
  const [installedModels, setInstalledModels] = useState<Set<string>>(new Set())
  const [flashMessage, setFlashMessage] = useState("")
  const [expandedIdx, setExpandedIdx] = useState<number>(-1)
  const [batchProcessing, setBatchProcessing] = useState(false)
  const [batchProgress, setBatchProgress] = useState({ current: 0, total: 0 })
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
        const active = await invoke<string>("ocr_get_active_model")
        setActiveModel(active)

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

  // Listen for screenshot OCR results
  useEffect(() => {
    const handler = (e: Event) => {
      const { text, timeMs, croppedImagePath, ocrResult } = (e as CustomEvent).detail
      if (text) {
        const screenshotItem: BatchOcrItem = {
          path: "",
          fileName: t("ocr.screenshotBtn"),
          imageUrl: croppedImagePath ? convertFileSrc(croppedImagePath) : "",
          state: "completed",
          result: ocrResult
            ? {
                textBlocks: ocrResult.textBlocks,
                totalTimeMs: ocrResult.totalTimeMs,
              }
            : {
                textBlocks: [{ text, confidence: 1.0, boxPoints: [] }],
                totalTimeMs: timeMs,
              },
          error: null,
        }
        setItems([screenshotItem])
        setExpandedIdx(0)
        showFlash(
          t("ocr.completedToast", {
            blocks: screenshotItem.result?.textBlocks.length ?? 1,
            time: (timeMs / 1000).toFixed(1),
          })
        )
      }
    }
    window.addEventListener("velocitext:screenshot-ocr-result", handler)
    return () => window.removeEventListener("velocitext:screenshot-ocr-result", handler)
  }, [showFlash, t])

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
            const imagePaths = paths.filter((p) => {
              const ext = p.split(".").pop()?.toLowerCase() ?? ""
              return IMAGE_EXTS.includes(ext)
            })
            if (imagePaths.length > 0) {
              addFiles(imagePaths)
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

  const addFiles = useCallback((paths: string[]) => {
    const newItems: BatchOcrItem[] = paths.map((p) => ({
      path: p,
      fileName: p.split(/[/\\]/).pop() || p,
      imageUrl: convertFileSrc(p),
      state: "loading" as OCRState,
      result: null,
      error: null,
    }))
    setItems((prev) => [...prev, ...newItems])
    setExpandedIdx(newItems.length === 1 ? 0 : -1)
    // Auto-start OCR for new items
    if (modelInstalled) {
      startBatchOCRForPaths(newItems.map((item) => item.path))
    }
  }, [modelInstalled])

  const handleOpenFile = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "Images",
            extensions: IMAGE_EXTS,
          },
        ],
      })
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected]
        addFiles(paths)
      }
    } catch (err) {
      console.error("Failed to open file:", err)
    }
  }

  const startBatchOCRForPaths = useCallback(async (paths: string[]) => {
    if (!modelInstalled || paths.length === 0) return

    setBatchProcessing(true)
    setBatchProgress({ current: 0, total: paths.length })

    let succeeded = 0
    let failed = 0
    let totalTimeMs = 0

    for (let i = 0; i < paths.length; i++) {
      const itemPath = paths[i]
      setBatchProgress({ current: i + 1, total: paths.length })

      // Mark item as loading
      setItems((prev) =>
        prev.map((it) =>
          it.path === itemPath ? { ...it, state: "loading" as OCRState, error: null } : it
        )
      )

      try {
        const res = await invoke<OcrResult>("ocr_recognize", {
          imagePath: itemPath,
          modelVersion: activeModel,
        })
        setItems((prev) =>
          prev.map((it) =>
            it.path === itemPath ? { ...it, state: "completed" as OCRState, result: res } : it
          )
        )
        succeeded++
        totalTimeMs += res.totalTimeMs
      } catch (err) {
        setItems((prev) =>
          prev.map((it) =>
            it.path === itemPath
              ? { ...it, state: "error" as OCRState, error: String(err) }
              : it
          )
        )
        failed++
      }
    }

    setBatchProcessing(false)
    showFlash(
      t("ocr.batchDone", {
        count: succeeded,
        time: (totalTimeMs / 1000).toFixed(1),
      })
    )
  }, [modelInstalled, activeModel, showFlash, t])

  const startBatchOCR = async () => {
    const pendingItems = items.filter((item) => item.state === "idle" || item.state === "error")
    if (pendingItems.length === 0) return
    await startBatchOCRForPaths(pendingItems.map((item) => item.path))
  }

  const handleModelChange = async (model: string) => {
    setActiveModel(model)
    try {
      await invoke("ocr_set_active_model", { modelName: model })

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

  const handleClear = () => {
    setItems([])
    setExpandedIdx(-1)
  }

  const handleRemoveItem = (idx: number) => {
    setItems((prev) => prev.filter((_, i) => i !== idx))
    if (expandedIdx === idx) setExpandedIdx(-1)
    else if (expandedIdx > idx) setExpandedIdx(expandedIdx - 1)
  }

  const handleCopyAllText = async () => {
    const allText = items
      .filter((item) => item.state === "completed" && item.result)
      .map((item) => item.result!.textBlocks.map((b) => b.text).join("\n"))
      .join("\n\n")
    if (!allText) return
    try {
      await invoke("copy_text_to_clipboard", { text: allText })
      showFlash(t("ocr.copyText"))
    } catch {
      await navigator.clipboard.writeText(allText)
      showFlash(t("ocr.copyText"))
    }
  }

  const handleCopyItemText = async (item: BatchOcrItem) => {
    if (!item.result) return
    const text = item.result.textBlocks.map((b) => b.text).join("\n")
    try {
      await invoke("copy_text_to_clipboard", { text })
      showFlash(t("ocr.copyText"))
    } catch {
      await navigator.clipboard.writeText(text)
      showFlash(t("ocr.copyText"))
    }
  }

  const handleExportAllTxt = async () => {
    const completedItems = items.filter((item) => item.state === "completed" && item.result)
    if (completedItems.length === 0) return

    if (completedItems.length === 1) {
      const item = completedItems[0]
      const text = item.result!.textBlocks.map((b) => b.text).join("\n")
      const baseName = item.path.replace(/\.[^.]+$/, "")
      const exportPath = `${baseName}_ocr.txt`
      try {
        await invoke("write_text_file", { path: exportPath, content: text })
        await invoke("open_file_with_system", { path: exportPath })
        showFlash(t("ocr.exportTxt"))
      } catch (err) {
        console.error("Export failed:", err)
      }
      return
    }

    // Batch export: one txt per image
    for (const item of completedItems) {
      const text = item.result!.textBlocks.map((b) => b.text).join("\n")
      const baseName = item.path.replace(/\.[^.]+$/, "")
      const exportPath = `${baseName}_ocr.txt`
      try {
        await invoke("write_text_file", { path: exportPath, content: text })
      } catch (err) {
        console.error("Export failed for:", item.fileName, err)
      }
    }
    showFlash(t("ocr.batchExportDone", { count: completedItems.length }))
  }

  const handleScreenshotOCR = async () => {
    if (!modelInstalled) {
      return
    }
    onScreenshotTrigger?.()
  }

  const hasItems = items.length > 0
  const hasCompleted = items.some((item) => item.state === "completed")
  const hasPending = items.some((item) => item.state === "idle" || item.state === "error")

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
              {MODEL_DISPLAY[key]}
              {installedModels.has(key) ? "" : " (not installed)"}
            </option>
          ))}
        </select>
        <Button
          onClick={handleScreenshotOCR}
          variant="default"
          size="sm"
          title={t("ocr.screenshotDesc")}
        >
          <CameraIcon className="size-4 mr-1" />
          {t("ocr.screenshotBtn")}
        </Button>
        <Button onClick={handleOpenFile} variant="outline" size="sm">
          <UploadIcon className="size-4 mr-1" />
          {t("ocr.selectImage")}
        </Button>
        {hasPending && !batchProcessing && (
          <Button onClick={startBatchOCR} variant="outline" size="sm" title={t("ocr.retryFailed")}>
            <ScanTextIcon className="size-4 mr-1" />
            {t("ocr.retryFailed")}
          </Button>
        )}
        {batchProcessing && (
          <Badge variant="secondary" className="h-9 px-3">
            {t("ocr.batchProgress", { current: batchProgress.current, total: batchProgress.total })}
          </Badge>
        )}
        {hasCompleted && (
          <Button onClick={handleCopyAllText} variant="outline" size="sm">
            <CopyIcon className="size-4 mr-1" />
            {t("ocr.copyAllText")}
          </Button>
        )}
        {hasCompleted && (
          <Button onClick={handleExportAllTxt} variant="outline" size="sm">
            <FileTextIcon className="size-4 mr-1" />
            {t("ocr.exportTxt")}
          </Button>
        )}
        <Button onClick={handleClear} variant="ghost" size="sm" disabled={!hasItems}>
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

      {!hasItems ? (
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
        /* Batch items list */
        <div className="space-y-2">
          {items.map((item, idx) => (
            <Card key={idx} className={expandedIdx === idx ? "ring-1 ring-primary/30" : ""}>
              {/* Item header - always visible */}
              <div
                className="flex items-center gap-3 p-3 cursor-pointer hover:bg-accent/30 transition-colors"
                onClick={() => setExpandedIdx(expandedIdx === idx ? -1 : idx)}
              >
                {expandedIdx === idx ? (
                  <ChevronDownIcon className="size-4 text-muted-foreground shrink-0" />
                ) : (
                  <ChevronRightIcon className="size-4 text-muted-foreground shrink-0" />
                )}

                {/* Thumbnail */}
                {item.imageUrl && (
                  <div className="size-10 rounded border bg-muted overflow-hidden shrink-0">
                    <img
                      src={item.imageUrl}
                      alt=""
                      className="w-full h-full object-cover"
                    />
                  </div>
                )}

                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium truncate">{item.fileName}</p>
                  <div className="flex items-center gap-2 mt-0.5">
                    {item.state === "loading" && (
                      <div className="flex items-center gap-1.5">
                        <div className="animate-spin rounded-full h-3 w-3 border-b-2 border-primary" />
                        <span className="text-xs text-muted-foreground">{t("ocr.processing")}</span>
                      </div>
                    )}
                    {item.state === "completed" && item.result && (
                      <div className="flex items-center gap-1.5">
                        <CheckCircleIcon className="size-3 text-green-600 dark:text-green-400" />
                        <span className="text-xs text-muted-foreground">
                          {t("ocr.textBlocks", { count: item.result.textBlocks.length })}
                          {" · "}
                          {(item.result.totalTimeMs / 1000).toFixed(1)}s
                        </span>
                      </div>
                    )}
                    {item.state === "error" && (
                      <div className="flex items-center gap-1.5">
                        <XCircleIcon className="size-3 text-red-500" />
                        <span className="text-xs text-red-500 truncate">{item.error}</span>
                      </div>
                    )}
                    {item.state === "idle" && (
                      <span className="text-xs text-muted-foreground">{t("ocr.pending")}</span>
                    )}
                  </div>
                </div>

                {/* Actions */}
                <div className="flex items-center gap-1 shrink-0">
                  {item.state === "completed" && (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7"
                      onClick={(e) => {
                        e.stopPropagation()
                        handleCopyItemText(item)
                      }}
                      title={t("ocr.copyText")}
                    >
                      <CopyIcon className="size-3.5" />
                    </Button>
                  )}
                  {!batchProcessing && (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7"
                      onClick={(e) => {
                        e.stopPropagation()
                        handleRemoveItem(idx)
                      }}
                      title={t("ocr.clear")}
                    >
                      <XCircleIcon className="size-3.5" />
                    </Button>
                  )}
                </div>
              </div>

              {/* Expanded detail */}
              {expandedIdx === idx && (
                <CardContent className="pt-0 pb-3">
                  <Separator className="mb-3" />
                  <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
                    {/* Image preview */}
                    {item.imageUrl && (
                      <div className="relative rounded-lg overflow-hidden bg-muted">
                        <img
                          src={item.imageUrl}
                          alt="Preview"
                          className="max-w-full max-h-[300px] object-contain mx-auto"
                        />
                      </div>
                    )}

                    {/* OCR result */}
                    <div>
                      {item.state === "loading" && (
                        <div className="flex items-center justify-center py-8">
                          <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary" />
                        </div>
                      )}

                      {item.state === "completed" && item.result && (
                        <div className="space-y-2">
                          {item.result.textBlocks.length === 0 ? (
                            <p className="text-sm text-muted-foreground text-center py-4">
                              {t("ocr.noTextFound")}
                            </p>
                          ) : (
                            <div className="space-y-1.5 max-h-[250px] overflow-y-auto">
                              {item.result.textBlocks.map((block, bIdx) => (
                                <div
                                  key={bIdx}
                                  className="p-2 rounded-lg border bg-muted/30 hover:bg-muted/50 transition-colors"
                                >
                                  <div className="flex items-start justify-between gap-2">
                                    <p className="text-sm leading-relaxed break-all">
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
                        </div>
                      )}

                      {item.state === "error" && (
                        <p className="text-sm text-red-600 dark:text-red-400 py-4">{item.error}</p>
                      )}

                      {item.state === "idle" && (
                        <p className="text-sm text-muted-foreground text-center py-4">
                          {t("ocr.pending")}
                        </p>
                      )}
                    </div>
                  </div>
                </CardContent>
              )}
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}
