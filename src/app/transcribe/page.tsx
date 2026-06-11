import { useState, useEffect, useRef, useCallback } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { Skeleton } from "@/components/ui/skeleton"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  UploadIcon,
  FileAudioIcon,
  FileVideoIcon,
  DownloadIcon,
  CopyIcon,
  Trash2Icon,
  PlusIcon,
  XIcon,
  SettingsIcon,
  SaveIcon,
} from "lucide-react"
import { convertFileSrc } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { useAppContext } from "@/lib/app-context"
import type { StreamingSegment, ProcessingState, InitStatus, VadSettings } from "@/types"

type TranscribeState = "idle" | "loading" | "processing" | "completed" | "cancelled" | "error"

function formatSrtTime(seconds: number): string {
  const totalMs = (seconds * 1000) as number
  const h = Math.floor(totalMs / 3_600_000)
  const m = Math.floor((totalMs % 3_600_000) / 60_000)
  const s = Math.floor((totalMs % 60_000) / 1000)
  const ms = Math.round(totalMs % 1000)
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")},${String(ms).padStart(3, "0")}`
}

/** Binary search to find segment index for a given time */
function findSegmentIndex(segments: StreamingSegment[], t: number): number {
  let lo = 0
  let hi = segments.length - 1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    const seg = segments[mid]
    if (t < seg.start) {
      hi = mid - 1
    } else if (t >= seg.end) {
      lo = mid + 1
    } else {
      return mid
    }
  }
  return -1
}

export function TranscribePage() {
  const { t } = useAppContext()
  const [transcribeState, setTranscribeState] = useState<TranscribeState>("loading")
  const [modelsReady, setModelsReady] = useState(false)
  const [modelError, setModelError] = useState<string | null>(null)
  const [progress, setProgress] = useState(0)
  const [segments, setSegments] = useState<StreamingSegment[]>([])
  const [fileName, setFileName] = useState<string | null>(null)
  const [filePath, setFilePath] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [isDragOver, setIsDragOver] = useState(false)
  const [elapsedSecs, setElapsedSecs] = useState<number | null>(null)
  const [audioDurationSecs, setAudioDurationSecs] = useState<number | null>(null)
  const [activeRowIdx, setActiveRowIdx] = useState(-1)
  const [activeSubtitle, setActiveSubtitle] = useState("")
  const [showSettings, setShowSettings] = useState(false)
  const [playerUrl, setPlayerUrl] = useState("")
  const [flashMessage, setFlashMessage] = useState("")
  const flashTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // VAD settings form state
  const [vadThreshold, setVadThreshold] = useState("0.2")
  const [vadMinSilence, setVadMinSilence] = useState("0.2")
  const [vadMinSpeech, setVadMinSpeech] = useState("0.2")
  const [vadMaxSpeech, setVadMaxSpeech] = useState("10.0")
  const [vadThreads, setVadThreads] = useState("2")
  const [settingsApplying, setSettingsApplying] = useState(false)

  // Player refs
  const playerRef = useRef<HTMLVideoElement>(null)
  const rafId = useRef<number | null>(null)
  const segmentsRef = useRef<StreamingSegment[]>([])
  const pollTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const initPollRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const tableBodyRef = useRef<HTMLTableSectionElement>(null)

  // Keep segmentsRef in sync
  useEffect(() => {
    segmentsRef.current = segments
  }, [segments])

  // Listen for Rust-side drag-drop events (Tauri v2 DragDropEvent)
  useEffect(() => {
    let unlistenDrop: UnlistenFn | null = null
    let unlistenHover: UnlistenFn | null = null

    const setup = async () => {
      unlistenDrop = await listen<string>("tauri://file-drop", (event) => {
        try {
          const paths: string[] = JSON.parse(event.payload)
          if (paths.length > 0 && modelsReady && transcribeState === "idle") {
            startRecognition(paths[0])
          }
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
  }, [modelsReady, transcribeState])

  // Compute player URL when filePath changes
  useEffect(() => {
    if (filePath) {
      setPlayerUrl(convertFileSrc(filePath))
    } else {
      setPlayerUrl("")
    }
  }, [filePath])

  // Flash message helper (like reference's flashStatus)
  const showFlash = (msg: string, durationMs = 2500) => {
    if (flashTimerRef.current) clearTimeout(flashTimerRef.current)
    setFlashMessage(msg)
    flashTimerRef.current = setTimeout(() => setFlashMessage(""), durationMs)
  }

  // ── Model initialization polling ────────────────────────────────────────
  const startInitPolling = useCallback(() => {
    const poll = setInterval(async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core")
        const res = await invoke<InitStatus>("get_init_status")
        if (res.status === 1) {
          setModelsReady(true)
          setTranscribeState("idle")
          clearInterval(poll)
        } else if (res.status === 2) {
          setModelError(res.error || "Unknown error")
          setTranscribeState("error")
          clearInterval(poll)
        }
        // status 0 — still loading, continue polling
      } catch (err) {
        setModelError(String(err))
        setTranscribeState("error")
        clearInterval(poll)
      }
    }, 300)
    initPollRef.current = poll
  }, [])

  useEffect(() => {
    startInitPolling()
    return () => {
      if (initPollRef.current) clearInterval(initPollRef.current)
      if (pollTimerRef.current) clearInterval(pollTimerRef.current)
      if (flashTimerRef.current) clearTimeout(flashTimerRef.current)
      if (rafId.current) cancelAnimationFrame(rafId.current)
    }
  }, [startInitPolling])

  // ── File selection ──────────────────────────────────────────────────────
  const openFileDialog = async (multiple: boolean) => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog")
      const selected = await open({
        multiple,
        filters: [
          {
            name: "Audio/Video",
            extensions: [
              "wav", "mp3", "flac", "ogg", "aac", "m4a", "aiff", "caf",
              "mp4", "mkv", "webm", "mov",
            ],
          },
        ],
      })

      if (selected) {
        if (Array.isArray(selected)) {
          if (selected.length > 0) startRecognition(selected[0])
        } else if (typeof selected === "string") {
          startRecognition(selected)
        }
      }
    } catch (err) {
      console.error("File dialog error:", err)
    }
  }

  // ── Streaming recognition ──────────────────────────────────────────────
  const startRecognition = async (path: string) => {
    if (!modelsReady) return

    // Reset UI
    setTranscribeState("processing")
    setProgress(0)
    setError(null)
    setElapsedSecs(null)
    setAudioDurationSecs(null)
    setActiveRowIdx(-1)
    setActiveSubtitle("")

    const name = path.split(/[\\/]/).pop() || path
    setFileName(name)
    setFilePath(path)
    setSegments([])
    segmentsRef.current = []

    // Stop any existing poll
    if (pollTimerRef.current) clearInterval(pollTimerRef.current)

    try {
      const { invoke } = await import("@tauri-apps/api/core")
      await invoke("recognize_file", { path })
      // Start polling for results
      startProgressPolling()
    } catch (err) {
      setError(String(err))
      setTranscribeState("error")
    }
  }

  const startProgressPolling = () => {
    pollTimerRef.current = setInterval(async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core")
        const state = await invoke<ProcessingState>("get_recognition_progress")

        setProgress(state.percent)

        // Append new segments
        const prevLen = segmentsRef.current.length
        if (state.segments.length > prevLen) {
          const newSegs = state.segments.slice(prevLen)
          setSegments((prev) => [...prev, ...newSegs])
          segmentsRef.current = state.segments
        }

        // Check terminal states
        if (state.status === "done") {
          clearInterval(pollTimerRef.current!)
          pollTimerRef.current = null
          setTranscribeState("completed")
          setProgress(100)
          setElapsedSecs(state.elapsedSecs)
          setAudioDurationSecs(state.audioDurationSecs)
          showFlash(
            t("transcribe.completedToast", {
              duration: state.audioDurationSecs.toFixed(1),
              elapsed: state.elapsedSecs.toFixed(1),
            }),
            4000
          )
        } else if (state.status === "cancelled") {
          clearInterval(pollTimerRef.current!)
          pollTimerRef.current = null
          setTranscribeState("cancelled")
        } else if (state.status.startsWith("error:")) {
          clearInterval(pollTimerRef.current!)
          pollTimerRef.current = null
          setError(state.status.slice(6))
          setTranscribeState("error")
        }
      } catch (err) {
        clearInterval(pollTimerRef.current!)
        pollTimerRef.current = null
        setError(String(err))
        setTranscribeState("error")
      }
    }, 200)
  }

  const handleCancel = async () => {
    const { invoke } = await import("@tauri-apps/api/core")
    await invoke("cancel_recognition")
  }

  // ── Player sync ─────────────────────────────────────────────────────────
  const startPlayerSync = () => {
    if (rafId.current) return
    setActiveRowIdx(-1)
    let lastActive = -1

    const tick = () => {
      const player = playerRef.current
      if (!player || player.paused || player.ended) {
        rafId.current = null
        return
      }

      const currentTime = player.currentTime
      const idx = findSegmentIndex(segmentsRef.current, currentTime)

      setActiveSubtitle(idx >= 0 ? segmentsRef.current[idx].text : "")

      // Update row highlight only on change
      if (idx !== lastActive) {
        setActiveRowIdx(idx)
        if (idx >= 0 && tableBodyRef.current) {
          const rows = tableBodyRef.current.querySelectorAll("tr")
          if (idx < rows.length) {
            rows[idx].scrollIntoView({ block: "nearest" })
          }
        }
        lastActive = idx
      }

      rafId.current = requestAnimationFrame(tick)
    }

    rafId.current = requestAnimationFrame(tick)
  }

  const stopPlayerSync = () => {
    if (rafId.current) {
      cancelAnimationFrame(rafId.current)
      rafId.current = null
    }
  }

  const handlePlayerPause = () => {
    stopPlayerSync()
    // Still show subtitle on pause
    const player = playerRef.current
    if (player) {
      const idx = findSegmentIndex(segmentsRef.current, player.currentTime)
      setActiveSubtitle(idx >= 0 ? segmentsRef.current[idx].text : "")
    }
  }

  const handlePlayerEnded = () => {
    stopPlayerSync()
    setActiveSubtitle("")
    setActiveRowIdx(-1)
  }

  // ── Row click → seek ───────────────────────────────────────────────────
  const handleRowClick = (segIdx: number) => {
    const player = playerRef.current
    if (!player || segIdx >= segmentsRef.current.length) return

    player.pause()
    const t = Math.max(0, segmentsRef.current[segIdx].start - 0.3)
    player.currentTime = t
    player.addEventListener(
      "seeked",
      () => {
        player.play().catch(() => {})
      },
      { once: true },
    )
  }

  // ── Save segment as WAV ────────────────────────────────────────────────
  const handleSaveSegment = async (e: React.MouseEvent, segIdx: number) => {
    e.stopPropagation()
    const seg = segmentsRef.current[segIdx]
    if (!seg || !filePath) return

    const { save } = await import("@tauri-apps/plugin-dialog")
    const { invoke } = await import("@tauri-apps/api/core")

    const start = seg.start.toFixed(2).replace(".", "_")
    const end = seg.end.toFixed(2).replace(".", "_")
    const textPart = seg.text.replace(/[^\w\u4e00-\u9fff]/g, "_").slice(0, 30)
    const defaultName = `segment-${segIdx + 1}-${start}s-${end}s-${textPart}.wav`

    const savePath = await save({
      defaultPath: defaultName,
      filters: [{ name: "WAV Audio", extensions: ["wav"] }],
    })

    if (!savePath) return

    try {
      await invoke("save_segment_as_wav", {
        path: savePath,
        start: seg.start,
        end: seg.end,
      })
      showFlash(`Saved: ${savePath.split(/[\\/]/).pop()}`)
    } catch (err) {
      setError(String(err))
    }
  }

  // ── Copy ───────────────────────────────────────────────────────────────
  const handleCopyText = () => {
    const text = segments.map((s) => s.text).join("")
    navigator.clipboard.writeText(text)
    showFlash("Text copied.")
  }

  const handleCopyTimed = () => {
    const lines = segments.map(
      (s) => `[${formatSrtTime(s.start)} --> ${formatSrtTime(s.end)}] ${s.text}`,
    )
    navigator.clipboard.writeText(lines.join("\n"))
    showFlash("Text with timestamps copied.")
  }

  // ── Export SRT ─────────────────────────────────────────────────────────
  const handleExportSrt = async () => {
    if (!filePath) return

    const { save } = await import("@tauri-apps/plugin-dialog")
    const { invoke } = await import("@tauri-apps/api/core")

    const baseName = filePath.split(/[\\/]/).pop() || "transcript"
    const nameWithoutExt = baseName.replace(/\.[^.]+$/, "")

    const savePath = await save({
      defaultPath: `${nameWithoutExt}.srt`,
      filters: [{ name: "SubRip Subtitle", extensions: ["srt"] }],
    })

    if (!savePath) return

    try {
      await invoke("export_to_file", {
        segments,
        format: "srt",
        savePath,
      })
    } catch (err) {
      setError(String(err))
    }
  }

  // ── Export TXT ─────────────────────────────────────────────────────────
  const handleExportTxt = async () => {
    if (!filePath) return

    const { save } = await import("@tauri-apps/plugin-dialog")
    const { invoke } = await import("@tauri-apps/api/core")

    const baseName = filePath.split(/[\\/]/).pop() || "transcript"
    const nameWithoutExt = baseName.replace(/\.[^.]+$/, "")

    const savePath = await save({
      defaultPath: `${nameWithoutExt}.txt`,
      filters: [{ name: "Text File", extensions: ["txt"] }],
    })

    if (!savePath) return

    try {
      await invoke("export_to_file", {
        segments,
        format: "txt",
        savePath,
      })
    } catch (err) {
      setError(String(err))
    }
  }

  // ── VAD Settings ────────────────────────────────────────────────────────
  const loadVadSettings = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      const s = await invoke<VadSettings>("get_vad_settings")
      setVadThreshold(String(s.threshold))
      setVadMinSilence(String(s.minSilenceDuration))
      setVadMinSpeech(String(s.minSpeechDuration))
      setVadMaxSpeech(String(s.maxSpeechDuration))
      setVadThreads(String(s.numThreads))
    } catch (err) {
      console.error("Failed to load VAD settings:", err)
    }
  }

  const handleApplySettings = async () => {
    const threshold = parseFloat(vadThreshold)
    const minSilence = parseFloat(vadMinSilence)
    const minSpeech = parseFloat(vadMinSpeech)
    const maxSpeech = parseFloat(vadMaxSpeech)
    const threads = parseInt(vadThreads, 10)

    if (isNaN(threshold) || threshold <= 0 || threshold >= 1) {
      setError("Threshold must be between 0.0 and 1.0 (exclusive)")
      return
    }
    if (isNaN(minSilence) || minSilence < 0) {
      setError("Min silence duration must be >= 0")
      return
    }
    if (isNaN(minSpeech) || minSpeech < 0) {
      setError("Min speech duration must be >= 0")
      return
    }
    if (isNaN(maxSpeech) || maxSpeech <= 0) {
      setError("Max speech duration must be > 0")
      return
    }
    if (isNaN(threads) || threads < 1 || threads > 16) {
      setError("Threads must be between 1 and 16")
      return
    }

    setSettingsApplying(true)
    setShowSettings(false)

    try {
      const { invoke } = await import("@tauri-apps/api/core")
      await invoke("apply_vad_settings", {
        newSettings: {
          threshold,
          minSilenceDuration: minSilence,
          minSpeechDuration: minSpeech,
          maxSpeechDuration: maxSpeech,
          numThreads: threads,
        },
      })
      // Models are being rebuilt — re-poll init status
      setModelsReady(false)
      setTranscribeState("loading")
      setError(null)

      // Restart init polling
      if (initPollRef.current) clearInterval(initPollRef.current)
      startInitPolling()
    } catch (err) {
      setError(String(err))
      setTranscribeState("idle")
      setModelsReady(true)
    } finally {
      setSettingsApplying(false)
    }
  }

  // ── Clear ──────────────────────────────────────────────────────────────
  const handleClear = () => {
    stopPlayerSync()
    if (pollTimerRef.current) clearInterval(pollTimerRef.current)
    pollTimerRef.current = null
    if (playerRef.current) {
      playerRef.current.src = ""
    }
    setTranscribeState("idle")
    setProgress(0)
    setSegments([])
    segmentsRef.current = []
    setFileName(null)
    setFilePath(null)
    setError(null)
    setElapsedSecs(null)
    setAudioDurationSecs(null)
    setActiveRowIdx(-1)
    setActiveSubtitle("")
    setPlayerUrl("")
    if (flashTimerRef.current) clearTimeout(flashTimerRef.current)
    setFlashMessage("")
  }

  const getFileIcon = () => {
    if (!fileName) return <UploadIcon className="size-12 text-muted-foreground" />
    const ext = fileName.split(".").pop()?.toLowerCase()
    const videoExts = ["mp4", "avi", "mov", "mkv", "flv", "webm"]
    return videoExts.includes(ext || "") ? (
      <FileVideoIcon className="size-12 text-blue-500" />
    ) : (
      <FileAudioIcon className="size-12 text-green-500" />
    )
  }

  // ── Render ──────────────────────────────────────────────────────────────
  return (
    <div className="px-4 lg:px-6 space-y-4">
      <Card>
        <CardHeader className="pb-2">
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="flex items-center gap-2">
                <UploadIcon className="size-5" />
                {t("transcribe.title")}
              </CardTitle>
              <CardDescription>{t("transcribe.desc")}</CardDescription>
            </div>
            {/* Settings gear button */}
            <Button
              variant="ghost"
              size="icon"
              disabled={!modelsReady || transcribeState === "processing"}
              title={t("transcribe.vadSettings")}
              onClick={() => {
                loadVadSettings()
                setShowSettings(true)
              }}
            >
              <SettingsIcon className="size-5" />
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {/* Flash message toast */}
          {flashMessage && (
            <div className="fixed bottom-6 right-6 z-50 px-4 py-2 bg-foreground text-background text-sm rounded-lg shadow-lg animate-in fade-in slide-in-from-bottom-2">
              {flashMessage}
            </div>
          )}

          {/* ── Loading models state ── */}
          {transcribeState === "loading" && (
            <div className="flex flex-col items-center justify-center py-12 gap-3">
              <Skeleton className="h-8 w-8 rounded-full" />
              <p className="text-sm text-muted-foreground">{t("transcribe.loadingModels")}</p>
            </div>
          )}

          {/* ── Idle state (drag & drop zone) ── */}
          {transcribeState === "idle" && (
            <div
              className={`flex flex-col items-center justify-center border-2 border-dashed rounded-lg p-12 cursor-pointer transition-colors ${
                isDragOver
                  ? "border-primary bg-primary/5"
                  : "border-muted-foreground/25 hover:border-primary/50"
              }`}
              onClick={() => openFileDialog(false)}
            >
              <div className="flex flex-col items-center gap-3">
                <UploadIcon
                  className={`size-10 ${isDragOver ? "text-primary" : "text-muted-foreground"}`}
                />
                <p className="text-sm text-muted-foreground">
                  {isDragOver ? t("transcribe.dropFiles") : t("transcribe.clickOrDrag")}
                </p>
                <Button variant="secondary" size="sm">
                  {t("transcribe.selectFile")}
                </Button>
              </div>
            </div>
          )}

          {/* ── Processing state ── */}
          {transcribeState === "processing" && (
            <div className="space-y-4 py-4">
              <div className="flex items-center gap-3">
                {getFileIcon()}
                <div className="flex-1">
                  <p className="font-medium text-sm">{fileName}</p>
                  <div className="flex items-center gap-2 mt-1.5">
                    <div className="flex-1 h-2.5 bg-muted rounded-full overflow-hidden">
                      <div
                        className="h-full bg-primary transition-all duration-300 ease-out rounded-full"
                        style={{ width: `${Math.max(progress, 3)}%` }}
                      />
                    </div>
                    <span className="text-xs text-muted-foreground min-w-[3ch] tabular-nums">
                      {progress}%
                    </span>
                  </div>
                  <p className="text-xs text-muted-foreground mt-1">
                    {segments.length > 0
                      ? `${t("transcribe.segmentsWithCount", { count: String(segments.length) })} · ${segments.reduce((sum, s) => sum + s.text.length, 0)} chars`
                      : t("transcribe.decoding")}
                  </p>
                </div>
              </div>

              {/* Cancel button */}
              <div className="flex justify-end">
                <Button variant="destructive" size="sm" onClick={handleCancel}>
                  <XIcon className="size-3.5 mr-1" />
                  {t("transcribe.cancel")}
                </Button>
              </div>

              {/* Player preview during processing */}
              {playerUrl && (
                <div className="sticky top-0 z-10 bg-background pb-2">
                  <div className="relative rounded-lg overflow-hidden bg-black">
                    {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
                    <video
                      ref={playerRef}
                      src={playerUrl}
                      controls
                      className="w-full max-h-[240px]"
                    />
                  </div>
                </div>
              )}

              {/* Live results table */}
              {segments.length > 0 && (
                <div className="border rounded-md overflow-hidden max-h-[400px] overflow-y-auto">
                  <table className="w-full text-sm">
                    <thead className="bg-muted/50 sticky top-0">
                      <tr>
                        <th className="text-left px-3 py-2 w-[80px]">Start</th>
                        <th className="text-left px-3 py-2 w-[80px]">End</th>
                        <th className="text-left px-3 py-2">Text</th>
                      </tr>
                    </thead>
                    <tbody ref={tableBodyRef}>
                      {segments.map((seg, i) => (
                        <tr
                          key={i}
                          className="border-t border-border hover:bg-accent/50 cursor-pointer transition-colors"
                          onClick={() => handleRowClick(i)}
                        >
                          <td className="px-3 py-1.5 text-muted-foreground tabular-nums">
                            {seg.start.toFixed(2)}s
                          </td>
                          <td className="px-3 py-1.5 text-muted-foreground tabular-nums">
                            {seg.end.toFixed(2)}s
                          </td>
                          <td className="px-3 py-1.5">{seg.text}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}

              {segments.length === 0 && (
                <div className="space-y-2">
                  <Skeleton className="h-4 w-3/4" />
                  <Skeleton className="h-4 w-1/2" />
                  <Skeleton className="h-4 w-2/3" />
                </div>
              )}
            </div>
          )}

          {/* ── Completed state ── */}
          {(transcribeState === "completed" || transcribeState === "cancelled") && (
            <div className="space-y-4">
              {/* Header row */}
              <div className="flex items-center justify-between flex-wrap gap-2">
                <div className="flex items-center gap-3">
                  {getFileIcon()}
                  <div>
                    <p className="font-medium text-sm">{fileName}</p>
                    <div className="flex items-center gap-2 mt-1 flex-wrap">
                      <Badge variant={transcribeState === "cancelled" ? "outline" : "secondary"}>
                        {transcribeState === "cancelled"
                          ? t("transcribe.cancelled")
                          : t("transcribe.done")}
                      </Badge>
                      {elapsedSecs != null && (
                        <span className="text-xs text-muted-foreground">
                          {elapsedSecs.toFixed(1)}s
                        </span>
                      )}
                      {audioDurationSecs != null && audioDurationSecs > 0 && (
                        <>
                          <span className="text-xs text-muted-foreground">
                            {t("transcribe.duration")} {audioDurationSecs.toFixed(1)}s
                          </span>
                          {elapsedSecs != null && elapsedSecs > 0 && (
                            <>
                              <span className="text-xs text-muted-foreground">
                                {t("transcribe.rtf")}: {(elapsedSecs / audioDurationSecs).toFixed(3)}
                              </span>
                              <Badge variant="outline" className="text-[10px]">
                                {t("transcribe.speedLabel", {
                                  speed: String((audioDurationSecs / elapsedSecs).toFixed(1)),
                                })}
                              </Badge>
                            </>
                          )}
                        </>
                      )}
                    </div>
                  </div>
                </div>

                {/* Action buttons */}
                <div className="flex items-center gap-2 flex-wrap">
                  <Button variant="outline" size="sm" onClick={handleCopyText}>
                    <CopyIcon className="size-3.5 mr-1" />
                    {t("transcribe.copyText")}
                  </Button>
                  <Button variant="outline" size="sm" onClick={handleCopyTimed}>
                    <CopyIcon className="size-3.5 mr-1" />
                    {t("transcribe.copyTimed")}
                  </Button>
                  <Button variant="outline" size="sm" onClick={handleExportSrt}>
                    <DownloadIcon className="size-3.5 mr-1" />
                    SRT
                  </Button>
                  <Button variant="outline" size="sm" onClick={handleExportTxt}>
                    <DownloadIcon className="size-3.5 mr-1" />
                    TXT
                  </Button>
                  <Button variant="secondary" size="sm" onClick={() => openFileDialog(false)}>
                    <PlusIcon className="size-3.5 mr-1" />
                    {t("transcribe.new")}
                  </Button>
                  <Button variant="ghost" size="sm" onClick={handleClear}>
                    <Trash2Icon className="size-3.5 mr-1" />
                    {t("transcribe.clear")}
                  </Button>
                </div>
              </div>

              {/* Built-in player — sticky to stay visible when scrolling segments */}
              {filePath && transcribeState === "completed" && playerUrl && (
                <div className="sticky top-0 z-10 bg-background pb-2">
                  <div className="relative rounded-lg overflow-hidden bg-black">
                    {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
                    <video
                      ref={playerRef}
                      src={playerUrl}
                      controls
                      className="w-full max-h-[320px]"
                      onPlay={startPlayerSync}
                      onPause={handlePlayerPause}
                      onEnded={handlePlayerEnded}
                    />
                    {/* Subtitle overlay — positioned above controls */}
                    {activeSubtitle && (
                      <div className="absolute bottom-10 left-1/2 -translate-x-1/2 max-w-[90%] px-3 py-1 bg-black/70 text-white text-sm rounded text-center pointer-events-none whitespace-pre-wrap">
                        {activeSubtitle}
                      </div>
                    )}
                  </div>
                </div>
              )}

              <Separator />

              {/* Segments table */}
              <div className="flex items-center justify-between">
                <p className="text-xs text-muted-foreground">
                  {t("transcribe.segmentsWithCount", { count: String(segments.length) })} ·{" "}
                  {segments.reduce((sum, s) => sum + s.text.length, 0)} chars
                </p>
              </div>

              <div className="border rounded-md overflow-hidden max-h-[500px] overflow-y-auto">
                <table className="w-full text-sm">
                  <thead className="bg-muted/50 sticky top-0">
                    <tr>
                      <th className="text-left px-3 py-2 w-[80px]">Start</th>
                      <th className="text-left px-3 py-2 w-[80px]">End</th>
                      <th className="text-left px-3 py-2">Text</th>
                      <th className="text-right px-3 py-2 w-[50px]"></th>
                    </tr>
                  </thead>
                  <tbody ref={tableBodyRef}>
                    {segments.map((seg, i) => (
                      <tr
                        key={i}
                        className={`border-t border-border hover:bg-accent/50 cursor-pointer transition-colors ${
                          i === activeRowIdx ? "bg-accent font-medium" : ""
                        }`}
                        onClick={() => handleRowClick(i)}
                      >
                        <td className="px-3 py-1.5 text-muted-foreground tabular-nums">
                          {seg.start.toFixed(2)}s
                        </td>
                        <td className="px-3 py-1.5 text-muted-foreground tabular-nums">
                          {seg.end.toFixed(2)}s
                        </td>
                        <td className="px-3 py-1.5">{seg.text}</td>
                        <td className="px-3 py-1.5 text-right">
                          <button
                            className="p-1 rounded hover:bg-muted transition-colors text-muted-foreground hover:text-foreground"
                            title={t("transcribe.saveSegment")}
                            onClick={(e) => handleSaveSegment(e, i)}
                          >
                            <SaveIcon className="size-3.5" />
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* ── Error state ── */}
          {transcribeState === "error" && (
            <div className="py-6 text-center space-y-3">
              {modelError && (
                <p className="text-destructive font-medium">{t("transcribe.initFailed")}</p>
              )}
              {error && (
                <p className="text-sm text-muted-foreground max-w-md mx-auto break-all">{error}</p>
              )}
              <div className="flex items-center justify-center gap-2">
                {modelsReady ? (
                  <>
                    <Button variant="outline" onClick={handleClear}>
                      {t("transcribe.tryAgain")}
                    </Button>
                    <Button variant="secondary" onClick={() => openFileDialog(false)}>
                      {t("transcribe.newFile")}
                    </Button>
                  </>
                ) : (
                  <Button variant="outline" onClick={() => window.location.reload()}>
                    Retry
                  </Button>
                )}
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* ── VAD Settings Modal ── */}
      {showSettings && (
        <div
          className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
          onClick={(e) => {
            if (e.target === e.currentTarget) setShowSettings(false)
          }}
        >
          <div className="bg-background rounded-xl shadow-lg p-6 max-w-[400px] w-[90%]">
            <h3 className="text-lg font-semibold mb-4">{t("transcribe.vadSettings")}</h3>

            <div className="space-y-4">
              <div>
                <Label htmlFor="vad-threshold">
                  {t("transcribe.vadThreshold")} (0.0–1.0)
                </Label>
                <Input
                  id="vad-threshold"
                  type="number"
                  min={0}
                  max={1}
                  step={0.05}
                  value={vadThreshold}
                  onChange={(e) => setVadThreshold(e.target.value)}
                />
              </div>
              <div>
                <Label htmlFor="vad-min-silence">{t("transcribe.vadMinSilence")}</Label>
                <Input
                  id="vad-min-silence"
                  type="number"
                  min={0}
                  step={0.1}
                  value={vadMinSilence}
                  onChange={(e) => setVadMinSilence(e.target.value)}
                />
              </div>
              <div>
                <Label htmlFor="vad-min-speech">{t("transcribe.vadMinSpeech")}</Label>
                <Input
                  id="vad-min-speech"
                  type="number"
                  min={0}
                  step={0.1}
                  value={vadMinSpeech}
                  onChange={(e) => setVadMinSpeech(e.target.value)}
                />
              </div>
              <div>
                <Label htmlFor="vad-max-speech">{t("transcribe.vadMaxSpeech")}</Label>
                <Input
                  id="vad-max-speech"
                  type="number"
                  min={0.5}
                  step={0.5}
                  value={vadMaxSpeech}
                  onChange={(e) => setVadMaxSpeech(e.target.value)}
                />
              </div>
              <div>
                <Label htmlFor="vad-threads">
                  {t("transcribe.vadThreads")} (1–16)
                </Label>
                <Input
                  id="vad-threads"
                  type="number"
                  min={1}
                  max={16}
                  step={1}
                  value={vadThreads}
                  onChange={(e) => setVadThreads(e.target.value)}
                />
              </div>
            </div>

            <div className="flex justify-end gap-2 mt-6">
              <Button variant="outline" onClick={() => setShowSettings(false)}>
                {t("transcribe.cancel")}
              </Button>
              <Button onClick={handleApplySettings} disabled={settingsApplying}>
                {settingsApplying ? t("transcribe.loadingModels") : t("transcribe.vadApply")}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}