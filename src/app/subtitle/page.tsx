import { useState, useEffect, useRef, useCallback } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Skeleton } from "@/components/ui/skeleton"
import {
  UploadIcon,
  FileAudioIcon,
  FileVideoIcon,
  PlusIcon,
  Trash2Icon,
  XIcon,
  CaptionsIcon,
  CheckCircleIcon,
  AlertCircleIcon,
} from "lucide-react"
import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { open } from "@tauri-apps/plugin-dialog"
import { useAppContext } from "@/lib/app-context"
import type { InitStatus, ProcessingState } from "@/types"

const MEDIA_EXTS = [
  "wav", "mp3", "flac", "ogg", "aac", "m4a", "aiff", "caf",
  "mp4", "mkv", "webm", "mov", "avi", "flv",
]

const VIDEO_EXTS = ["mp4", "avi", "mov", "mkv", "flv", "webm"]

type ItemStatus = "pending" | "processing" | "writing" | "done" | "failed" | "cancelled"

interface SubtitleItem {
  path: string
  fileName: string
  status: ItemStatus
  progress: number
  error: string | null
}

/** Build the SRT output path: same folder as the source file, same base name. */
function buildSrtPath(path: string): string {
  const name = path.split(/[\\/]/).pop() || "subtitle.srt"
  const base = name.replace(/\.[^.]+$/, "")
  const dir = path.slice(0, path.length - name.length)
  return `${dir}${base}.srt`
}

const isMediaFile = (p: string) =>
  MEDIA_EXTS.includes(p.split(".").pop()?.toLowerCase() ?? "")

function getFileIcon(fileName: string) {
  const ext = fileName.split(".").pop()?.toLowerCase() ?? ""
  return VIDEO_EXTS.includes(ext) ? (
    <FileVideoIcon className="size-4 text-blue-500 shrink-0" />
  ) : (
    <FileAudioIcon className="size-4 text-green-500 shrink-0" />
  )
}

export function SubtitlePage() {
  const { t } = useAppContext()
  const [modelsReady, setModelsReady] = useState(false)
  const [modelError, setModelError] = useState<string | null>(null)
  const [items, setItems] = useState<SubtitleItem[]>([])
  const [batchRunning, setBatchRunning] = useState(false)
  const [isDragOver, setIsDragOver] = useState(false)
  const [flashMessage, setFlashMessage] = useState("")
  const [currentIdx, setCurrentIdx] = useState(-1)

  const flashTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const initPollRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const pollTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const stopRef = useRef(false)
  const itemsRef = useRef<SubtitleItem[]>([])
  const modelsReadyRef = useRef(false)

  useEffect(() => {
    itemsRef.current = items
  }, [items])

  useEffect(() => {
    modelsReadyRef.current = modelsReady
  }, [modelsReady])

  const showFlash = useCallback((msg: string, durationMs = 3000) => {
    if (flashTimerRef.current) clearTimeout(flashTimerRef.current)
    setFlashMessage(msg)
    flashTimerRef.current = setTimeout(() => setFlashMessage(""), durationMs)
  }, [])

  // ── Model initialization (lazy loading, same as Transcribe page) ────────
  const startInitPolling = useCallback(() => {
    const poll = setInterval(async () => {
      try {
        const res = await invoke<InitStatus>("get_init_status")
        if (res.status === 1) {
          setModelsReady(true)
          clearInterval(poll)
        } else if (res.status === 2) {
          setModelError(res.error || "Unknown error")
          clearInterval(poll)
        }
      } catch (err) {
        setModelError(String(err))
        clearInterval(poll)
      }
    }, 300)
    initPollRef.current = poll
  }, [])

  useEffect(() => {
    const loadModels = async () => {
      try {
        const res = await invoke<InitStatus>("ensure_asr_models")
        if (res.status === 1) {
          setModelsReady(true)
        } else if (res.status === 0) {
          startInitPolling()
        } else if (res.status === 2) {
          setModelError(res.error || "Unknown error")
        }
      } catch (err) {
        setModelError(String(err))
      }
    }
    loadModels()

    return () => {
      if (initPollRef.current) clearInterval(initPollRef.current)
      if (pollTimerRef.current) clearInterval(pollTimerRef.current)
      if (flashTimerRef.current) clearTimeout(flashTimerRef.current)

      // Release ASR models when leaving the page (no-op while recognition runs)
      invoke("release_asr_models").catch(() => {
        // ignore (might be running or already released)
      })
    }
  }, [startInitPolling])

  // ── Drag & drop ─────────────────────────────────────────────────────────
  useEffect(() => {
    let unlistenDrop: UnlistenFn | null = null
    let unlistenHover: UnlistenFn | null = null

    const setup = async () => {
      unlistenDrop = await listen<string>("tauri://file-drop", (event) => {
        try {
          const paths: string[] = JSON.parse(event.payload)
          addFiles(paths.filter(isMediaFile))
        } catch { /* ignore parse errors */ }
        setIsDragOver(false)
      })

      unlistenHover = await listen<boolean>("tauri://file-drop-hover", (event) => {
        setIsDragOver(event.payload)
      })
    }

    setup()

    return () => {
      unlistenDrop?.()
      unlistenHover?.()
    }
  }, [])

  // ── File selection ──────────────────────────────────────────────────────
  const addFiles = (paths: string[]) => {
    if (batchRunning) return
    setItems((prev) => {
      const existing = new Set(prev.map((it) => it.path))
      const fresh = paths
        .filter((p) => !existing.has(p))
        .map((p) => ({
          path: p,
          fileName: p.split(/[\\/]/).pop() || p,
          status: "pending" as const,
          progress: 0,
          error: null,
        }))
      return [...prev, ...fresh]
    })
  }

  const openFileDialog = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "Audio/Video",
            extensions: MEDIA_EXTS,
          },
        ],
      })

      if (selected) {
        if (Array.isArray(selected)) {
          addFiles(selected)
        } else if (typeof selected === "string") {
          addFiles([selected])
        }
      }
    } catch (err) {
      console.error("File dialog error:", err)
    }
  }

  const removeItem = (idx: number) => {
    if (batchRunning) return
    setItems((prev) => prev.filter((_, i) => i !== idx))
  }

  const handleClear = () => {
    if (batchRunning) return
    setItems([])
    showFlash("")
  }

  const updateItem = (idx: number, patch: Partial<SubtitleItem>) => {
    setItems((prev) => prev.map((it, i) => (i === idx ? { ...it, ...patch } : it)))
  }

  // ── Recognition polling for a single file ────────────────────────────────
  const pollRecognition = (idx: number): Promise<ProcessingState> => {
    return new Promise((resolve, reject) => {
      const timer = setInterval(async () => {
        try {
          const state = await invoke<ProcessingState>("get_recognition_progress")
          updateItem(idx, { progress: state.percent })

          if (state.status === "done") {
            clearInterval(timer)
            pollTimerRef.current = null
            resolve(state)
          } else if (state.status === "cancelled") {
            clearInterval(timer)
            pollTimerRef.current = null
            reject(new Error("cancelled"))
          } else if (state.status.startsWith("error:")) {
            clearInterval(timer)
            pollTimerRef.current = null
            reject(new Error(state.status.slice(6)))
          }
        } catch (err) {
          clearInterval(timer)
          pollTimerRef.current = null
          reject(err)
        }
      }, 200)
      pollTimerRef.current = timer
    })
  }

  // ── Batch subtitle generation ────────────────────────────────────────────
  const startBatch = async () => {
    if (!modelsReadyRef.current) return
    if (itemsRef.current.length === 0) {
      showFlash(t("subtitle.noFiles"))
      return
    }

    // Reset every item (re-generates done ones too) and capture a stable working list
    const list = itemsRef.current.map((it) => ({
      ...it,
      status: "pending" as const,
      progress: 0,
      error: null,
    }))
    setItems(list)
    itemsRef.current = list

    stopRef.current = false
    setBatchRunning(true)
    const batchStart = Date.now()
    let succeeded = 0
    let failed = 0

    for (let i = 0; i < list.length; i++) {
      if (stopRef.current) break

      const item = list[i]
      setCurrentIdx(i)
      updateItem(i, { status: "processing", progress: 0 })

      try {
        await invoke("recognize_file", { path: item.path })
        const state = await pollRecognition(i)

        // Write SRT next to the source file
        updateItem(i, { status: "writing" })
        const srtPath = buildSrtPath(item.path)
        await invoke("export_to_file", {
          segments: state.segments,
          format: "srt",
          savePath: srtPath,
        })

        updateItem(i, { status: "done", progress: 100 })
        succeeded++
      } catch (err) {
        const msg = String(err instanceof Error ? err.message : err)
        if (msg === "cancelled" || stopRef.current) {
          updateItem(i, { status: "cancelled" })
          break
        }
        updateItem(i, { status: "failed", error: msg })
        failed++
      }
    }

    setCurrentIdx(-1)
    setBatchRunning(false)

    if (!stopRef.current) {
      const time = ((Date.now() - batchStart) / 1000).toFixed(1)
      showFlash(
        t("subtitle.completedToast", {
          succeeded: String(succeeded),
          failed: String(failed),
          time,
        }),
        5000,
      )
    }
  }

  const handleStop = async () => {
    stopRef.current = true
    try {
      await invoke("cancel_recognition")
    } catch {
      // ignore — nothing running
    }
  }

  // ── Render helpers ──────────────────────────────────────────────────────
  const doneCount = items.filter((it) => it.status === "done").length
  const overallPercent =
    items.length > 0 ? Math.round((doneCount / items.length) * 100) : 0

  const statusBadge = (item: SubtitleItem) => {
    switch (item.status) {
      case "done":
        return (
          <Badge variant="outline" className="gap-1 border-green-300 dark:border-green-700">
            <CheckCircleIcon className="size-3 text-green-600 dark:text-green-400" />
            <span className="text-green-700 dark:text-green-300">{t("subtitle.done")}</span>
          </Badge>
        )
      case "failed":
        return (
          <Badge variant="destructive" className="gap-1">
            <AlertCircleIcon className="size-3" />
            {t("subtitle.failed")}
          </Badge>
        )
      case "cancelled":
        return <Badge variant="outline">{t("subtitle.cancelled")}</Badge>
      case "writing":
        return <Badge variant="secondary">{t("subtitle.writing")}</Badge>
      case "processing":
        return (
          <span className="text-xs text-muted-foreground tabular-nums">
            {t("subtitle.processing", { percent: String(item.progress) })}
          </span>
        )
      default:
        return <span className="text-xs text-muted-foreground">{t("subtitle.pending")}</span>
    }
  }

  // ── Render ──────────────────────────────────────────────────────────────
  return (
    <div className="px-4 lg:px-6 space-y-4">
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="flex items-center gap-2">
            <CaptionsIcon className="size-5" />
            {t("subtitle.title")}
          </CardTitle>
          <CardDescription>{t("subtitle.desc")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {/* Flash message toast */}
          {flashMessage && (
            <div className="fixed bottom-6 right-6 z-50 px-4 py-2 bg-foreground text-background text-sm rounded-lg shadow-lg animate-in fade-in slide-in-from-bottom-2">
              {flashMessage}
            </div>
          )}

          {/* Loading models */}
          {!modelsReady && !modelError && (
            <div className="flex flex-col items-center justify-center py-12 gap-3">
              <Skeleton className="h-8 w-8 rounded-full" />
              <p className="text-sm text-muted-foreground">{t("subtitle.loadingModels")}</p>
            </div>
          )}

          {/* Model init error */}
          {modelError && (
            <div className="py-6 text-center space-y-3">
              <p className="text-destructive font-medium">{t("subtitle.initFailed")}</p>
              <p className="text-sm text-muted-foreground max-w-md mx-auto break-all">{modelError}</p>
              <Button variant="outline" onClick={() => window.location.reload()}>
                {t("transcribe.tryAgain")}
              </Button>
            </div>
          )}

          {modelsReady && (
            <>
              {/* Drop zone */}
              {items.length === 0 ? (
                <div
                  className={`flex flex-col items-center justify-center border-2 border-dashed rounded-lg p-12 cursor-pointer transition-colors ${
                    isDragOver
                      ? "border-primary bg-primary/5"
                      : "border-muted-foreground/25 hover:border-primary/50"
                  }`}
                  onClick={() => !batchRunning && openFileDialog()}
                >
                  <div className="flex flex-col items-center gap-3">
                    <UploadIcon
                      className={`size-10 ${isDragOver ? "text-primary" : "text-muted-foreground"}`}
                    />
                    <p className="text-sm text-muted-foreground">
                      {isDragOver ? t("subtitle.dropFiles") : t("subtitle.clickOrDrag")}
                    </p>
                    <Button variant="secondary" size="sm">
                      {t("subtitle.selectFiles")}
                    </Button>
                  </div>
                </div>
              ) : (
                <>
                  {/* Overall progress */}
                  {batchRunning && (
                    <div className="space-y-1.5">
                      <div className="flex items-center justify-between text-xs text-muted-foreground">
                        <span>
                          {t("subtitle.fileCount", { count: String(currentIdx + 1) })} /{" "}
                          {t("subtitle.fileCount", { count: String(items.length) })}
                        </span>
                        <span className="tabular-nums">{overallPercent}%</span>
                      </div>
                      <div className="h-2 bg-muted rounded-full overflow-hidden">
                        <div
                          className="h-full bg-primary transition-all duration-300 ease-out rounded-full"
                          style={{ width: `${Math.max(overallPercent, 2)}%` }}
                        />
                      </div>
                    </div>
                  )}

                  {/* File list — scrolls internally so action buttons stay visible */}
                  <div className="border rounded-md max-h-[50vh] overflow-y-auto">
                    <table className="w-full text-sm">
                      <thead className="bg-muted/50 sticky top-0">
                        <tr>
                          <th className="text-left px-3 py-2">{t("subtitle.file")}</th>
                          <th className="text-left px-3 py-2 w-[140px]">{t("subtitle.status")}</th>
                          <th className="text-right px-3 py-2 w-[100px]"></th>
                        </tr>
                      </thead>
                      <tbody>
                        {items.map((item, i) => (
                          <tr
                            key={item.path}
                            className={`border-t border-border transition-colors ${
                              i === currentIdx && batchRunning ? "bg-accent/50" : "hover:bg-accent/30"
                            }`}
                          >
                            <td className="px-3 py-1.5">
                              <div className="flex items-center gap-2 min-w-0">
                                {getFileIcon(item.fileName)}
                                <span className="truncate" title={item.path}>
                                  {item.fileName}
                                </span>
                              </div>
                              {item.error && (
                                <p className="text-xs text-destructive mt-0.5 break-all" title={item.error}>
                                  {item.error}
                                </p>
                              )}
                            </td>
                            <td className="px-3 py-1.5">
                              {statusBadge(item)}
                            </td>
                            <td className="px-3 py-1.5 text-right">
                              {!batchRunning && (
                                <button
                                  className="p-1 rounded hover:bg-muted transition-colors text-muted-foreground hover:text-foreground"
                                  title={t("subtitle.remove")}
                                  onClick={() => removeItem(i)}
                                >
                                  <XIcon className="size-3.5" />
                                </button>
                              )}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center justify-between flex-wrap gap-2">
                    <div className="flex items-center gap-2">
                      {batchRunning ? (
                        <Button variant="destructive" size="sm" onClick={handleStop}>
                          <XIcon className="size-3.5 mr-1" />
                          {t("subtitle.stop")}
                        </Button>
                      ) : (
                        <>
                          <Button size="sm" onClick={startBatch}>
                            <CaptionsIcon className="size-3.5 mr-1" />
                            {t("subtitle.start")}
                          </Button>
                          <Button variant="outline" size="sm" onClick={() => openFileDialog()}>
                            <PlusIcon className="size-3.5 mr-1" />
                            {t("subtitle.addMore")}
                          </Button>
                          <Button variant="ghost" size="sm" onClick={handleClear}>
                            <Trash2Icon className="size-3.5 mr-1" />
                            {t("subtitle.clear")}
                          </Button>
                        </>
                      )}
                    </div>
                    <span className="text-xs text-muted-foreground">
                      {t("subtitle.fileCount", { count: String(items.length) })}
                    </span>
                  </div>
                </>
              )}
            </>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
