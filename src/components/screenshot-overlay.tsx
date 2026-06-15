import { useState, useRef, useCallback, useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { convertFileSrc } from "@tauri-apps/api/core"

interface ScreenshotOverlayProps {
  imagePath: string
  width: number
  height: number
  modelVersion: string
  onComplete: (text: string, timeMs: number) => void
  onCancel: () => void
}

export function ScreenshotOverlay({
  imagePath,
  modelVersion,
  onComplete,
  onCancel,
}: ScreenshotOverlayProps) {
  const [selecting, setSelecting] = useState(false)
  const [selected, setSelected] = useState(false)
  const [rect, setRect] = useState({ x: 0, y: 0, w: 0, h: 0 })
  const [processing, setProcessing] = useState(false)
  const startRef = useRef({ x: 0, y: 0 })
  const overlayRef = useRef<HTMLDivElement>(null)

  const imageUrl = convertFileSrc(imagePath)

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (processing) return
      const rect = overlayRef.current?.getBoundingClientRect()
      if (!rect) return
      const x = e.clientX - rect.left
      const y = e.clientY - rect.top
      startRef.current = { x, y }
      setSelecting(true)
      setSelected(false)
      setRect({ x, y, w: 0, h: 0 })
    },
    [processing]
  )

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (!selecting || processing) return
      const containerRect = overlayRef.current?.getBoundingClientRect()
      if (!containerRect) return
      const x = e.clientX - containerRect.left
      const y = e.clientY - containerRect.top
      const start = startRef.current
      setRect({
        x: Math.min(start.x, x),
        y: Math.min(start.y, y),
        w: Math.abs(x - start.x),
        h: Math.abs(y - start.y),
      })
    },
    [selecting, processing]
  )

  const handleMouseUp = useCallback(async () => {
    if (!selecting || processing) return
    setSelecting(false)
    // Only accept selections with reasonable size (> 10px)
    if (rect.w < 10 || rect.h < 10) return
    setSelected(true)
    setProcessing(true)

    try {
      const result = await invoke<{
        textBlocks: Array<{ text: string; confidence: number }>
        totalTimeMs: number
      }>("ocr_screenshot_region", {
        imagePath,
        x: Math.round(rect.x),
        y: Math.round(rect.y),
        width: Math.round(rect.w),
        height: Math.round(rect.h),
        modelVersion,
      })

      const text = result.textBlocks.map((b) => b.text).join("\n")
      if (text) {
        await invoke("copy_text_to_clipboard", { text })
      }
      onComplete(text, result.totalTimeMs)
    } catch (err) {
      console.error("Screenshot OCR failed:", err)
      onComplete("", 0)
    }
  }, [selecting, processing, rect, imagePath, modelVersion, onComplete])

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (processing) return
        onCancel()
      }
    }
    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [onCancel, processing])

  return (
    <div
      ref={overlayRef}
      className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center"
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      style={{ cursor: processing ? "wait" : "crosshair" }}
    >
      <div className="relative" style={{ maxWidth: "100vw", maxHeight: "100vh" }}>
        <img
          src={imageUrl}
          alt="Screenshot"
          className="max-w-screen max-h-screen object-contain"
          draggable={false}
          style={{ pointerEvents: "none" }}
          onLoad={(e) => {
            // Scale image to fit screen
            const img = e.currentTarget
            const scale = Math.min(
              window.innerWidth / img.naturalWidth,
              window.innerHeight / img.naturalHeight
            )
            img.style.width = `${img.naturalWidth * scale}px`
            img.style.height = `${img.naturalHeight * scale}px`
          }}
        />

        {/* Selection rectangle */}
        {((selecting && rect.w > 0 && rect.h > 0) || (selected && rect.w >= 10 && rect.h >= 10)) && (
          <div
            className="absolute border-2 border-blue-400 bg-blue-400/20 pointer-events-none"
            style={{
              left: rect.x,
              top: rect.y,
              width: rect.w,
              height: rect.h,
            }}
          />
        )}

        {/* Processing indicator */}
        {processing && (
          <div className="absolute inset-0 flex items-center justify-center bg-black/30">
            <div className="bg-background rounded-lg px-6 py-3 shadow-lg text-sm font-medium">
              正在识别...
            </div>
          </div>
        )}
      </div>

      {/* Hint text */}
      <div className="absolute bottom-6 left-1/2 -translate-x-1/2 text-white/70 text-sm bg-black/50 rounded-full px-4 py-1.5">
        拖拽选择识别区域 · ESC 取消
      </div>
    </div>
  )
}