import {
  useState,
  useEffect,
  useRef,
  useCallback,
  useImperativeHandle,
  forwardRef,
} from "react"
import WaveSurfer from "wavesurfer.js"
import { PlayIcon, PauseIcon } from "lucide-react"
import { Button } from "@/components/ui/button"

function formatTime(sec: number): string {
  if (!isFinite(sec) || sec < 0) return "00:00"
  const m = Math.floor(sec / 60)
  const s = Math.floor(sec % 60)
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
}

export interface WaveformPlayerHandle {
  play: () => void
  pause: () => void
  seekTo: (time: number) => void
  getCurrentTime: () => number
  getDuration: () => number
}

interface WaveformPlayerProps {
  url: string
  subtitle?: string
  onTimeUpdate?: (time: number) => void
  onPlay?: () => void
  onPause?: () => void
  onEnded?: () => void
  onReady?: () => void
}

export const WaveformPlayer = forwardRef<WaveformPlayerHandle, WaveformPlayerProps>(
  ({ url, subtitle, onTimeUpdate, onPlay, onPause, onEnded, onReady }, ref) => {
    const containerRef = useRef<HTMLDivElement>(null)
    const wsRef = useRef<WaveSurfer | null>(null)
    const [isPlaying, setIsPlaying] = useState(false)
    const [currentTime, setCurrentTime] = useState(0)
    const [duration, setDuration] = useState(0)
    const [isReady, setIsReady] = useState(false)

    // Initialize / re-create wavesurfer when url changes
    useEffect(() => {
      if (!containerRef.current || !url) return

      // Destroy previous instance
      if (wsRef.current) {
        wsRef.current.destroy()
        wsRef.current = null
      }

      setIsReady(false)
      setCurrentTime(0)
      setDuration(0)
      setIsPlaying(false)

      const ws = WaveSurfer.create({
        container: containerRef.current,
        waveColor: "rgba(148, 163, 184, 0.4)",
        progressColor: "hsl(var(--primary))",
        cursorColor: "hsl(var(--primary))",
        cursorWidth: 2,
        height: 80,
        barWidth: 2,
        barGap: 1,
        barRadius: 2,
        normalize: true,
        url,
      })

      ws.on("ready", () => {
        setDuration(ws.getDuration())
        setIsReady(true)
        onReady?.()
      })

      ws.on("play", () => {
        setIsPlaying(true)
        onPlay?.()
      })

      ws.on("pause", () => {
        setIsPlaying(false)
        onPause?.()
      })

      ws.on("finish", () => {
        setIsPlaying(false)
        onEnded?.()
      })

      ws.on("timeupdate", (time) => {
        setCurrentTime(time)
        onTimeUpdate?.(time)
      })

      wsRef.current = ws

      return () => {
        ws.destroy()
        wsRef.current = null
      }
    }, [url]) // eslint-disable-line react-hooks/exhaustive-deps

    // Expose imperative handle
    useImperativeHandle(ref, () => ({
      play: () => wsRef.current?.play(),
      pause: () => wsRef.current?.pause(),
      seekTo: (time: number) => {
        if (wsRef.current) wsRef.current.setTime(time)
      },
      getCurrentTime: () => wsRef.current?.getCurrentTime() ?? 0,
      getDuration: () => wsRef.current?.getDuration() ?? 0,
    }))

    const togglePlay = useCallback(() => {
      wsRef.current?.playPause()
    }, [])

    return (
      <div className="rounded-lg overflow-hidden bg-black/5 dark:bg-black/20">
        {/* Waveform */}
        <div className="relative">
          <div ref={containerRef} className="waveform-container" />

          {/* Subtitle overlay */}
          {subtitle && (
            <div className="absolute bottom-2 left-1/2 -translate-x-1/2 max-w-[90%] px-3 py-1 bg-black/70 text-white text-sm rounded text-center pointer-events-none whitespace-pre-wrap">
              {subtitle}
            </div>
          )}
        </div>

        {/* Controls bar */}
        <div className="flex items-center gap-3 px-3 py-2 bg-background/80 border-t">
          <Button
            variant="ghost"
            size="icon"
            className="size-8 shrink-0"
            onClick={togglePlay}
            disabled={!isReady}
          >
            {isPlaying ? (
              <PauseIcon className="size-4" />
            ) : (
              <PlayIcon className="size-4" />
            )}
          </Button>

          <span className="text-xs text-muted-foreground tabular-nums min-w-[90px]">
            {formatTime(currentTime)} / {formatTime(duration)}
          </span>

          {/* Loading indicator */}
          {!isReady && url && (
            <span className="text-xs text-muted-foreground animate-pulse">
              Loading...
            </span>
          )}
        </div>
      </div>
    )
  },
)

WaveformPlayer.displayName = "WaveformPlayer"
