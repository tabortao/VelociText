import { useState, useEffect, useCallback, useRef } from "react"
import { AppSidebar } from "@/components/app-sidebar"
import {
  SidebarInset,
  SidebarProvider,
} from "@/components/ui/sidebar"
import { TooltipProvider } from "@/components/ui/tooltip"
import { SiteHeader } from "@/components/site-header"
import { TranscribePage } from "@/app/transcribe/page"
import { DictionaryPage } from "@/app/dictionary/page"
import { OCRPage } from "@/app/ocr/page"
import { SettingsPage } from "@/app/settings/page"
import { ModelSettingsPage } from "@/app/settings/model-settings"
import { AboutPage } from "@/app/about/page"
import { AppProvider } from "@/lib/app-context"
import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import type { AppConfig } from "@/types"

export type Page = "transcribe" | "dictionary" | "ocr" | "settings" | "model-settings" | "about"

export default function App() {
  const [currentPage, setCurrentPage] = useState<Page>("transcribe")
  const [sidebarOpen, setSidebarOpen] = useState(true)
  const [ocrModelVersion, setOcrModelVersion] = useState("ppocr-v5")
  const unlistenRef = useRef<UnlistenFn | null>(null)

  // Load sidebar state from config on mount
  useEffect(() => {
    const load = async () => {
      try {
        const config = await invoke<AppConfig>("get_app_config")
        setSidebarOpen(!config.sidebarCollapsed)
      } catch {
        // Use default (open)
      }
    }
    load()
  }, [])

  // Load OCR model version
  useEffect(() => {
    const load = async () => {
      try {
        const model = await invoke<string>("ocr_get_active_model")
        setOcrModelVersion(model)
      } catch {
        // ignore
      }
    }
    load()
  }, [])

  // Listen for screenshot OCR result events from the main window
  // (Rust emits this to the main window after the screenshot window completes OCR)
  useEffect(() => {
    let unlistenResult: UnlistenFn | undefined

    const setup = async () => {
      try {
        unlistenResult = await listen<{
          text: string
          timeMs: number
        }>("screenshot-ocr-result", (event) => {
          const { text, timeMs } = event.payload
          // Navigate to OCR page to show results
          setCurrentPage("ocr")
          // The OCR page will receive the result via a custom event
          window.dispatchEvent(
            new CustomEvent("velocitext:screenshot-ocr-result", {
              detail: { text, timeMs },
            })
          )
        })
      } catch {
        // ignore
      }
    }
    setup()

    return () => {
      unlistenResult?.()
    }
  }, [])

  // Register global shortcut for screenshot OCR
  useEffect(() => {
    let cancelled = false

    const register = async () => {
      try {
        const { register: registerShortcut, unregister } = await import(
          "@tauri-apps/plugin-global-shortcut"
        )

        // Load the configured shortcut from config
        let shortcut = "Ctrl+Shift+O"
        try {
          const config = await invoke<AppConfig>("get_app_config")
          if (config.ocrScreenshotShortcut) {
            shortcut = config.ocrScreenshotShortcut
          }
        } catch {
          // use default
        }

        // Unregister any previous shortcut
        try {
          await unregister(shortcut)
        } catch {
          // might not be registered
        }

        await registerShortcut(shortcut, async (event) => {
          if (event.state === "Pressed" && !cancelled) {
            triggerScreenshot()
          }
        })

        unlistenRef.current = () => {
          unregister(shortcut).catch(() => {})
        }
      } catch (err) {
        console.error("Failed to register global shortcut:", err)
      }
    }

    register()

    return () => {
      cancelled = true
      unlistenRef.current?.()
    }
  }, [])

  // Trigger screenshot: capture + open transparent fullscreen window
  const triggerScreenshot = useCallback(async () => {
    try {
      // Refresh OCR model version
      try {
        const model = await invoke<string>("ocr_get_active_model")
        setOcrModelVersion(model)
      } catch {
        // keep current
      }

      await invoke("start_screenshot_selection", { modelVersion: ocrModelVersion })
    } catch (err) {
      console.error("Screenshot capture failed:", err)
    }
  }, [ocrModelVersion])

  // Persist sidebar state to config
  const handleSidebarOpenChange = useCallback(async (open: boolean) => {
    setSidebarOpen(open)
    try {
      const config = await invoke<AppConfig>("get_app_config")
      config.sidebarCollapsed = !open
      await invoke("set_app_config", { newConfig: config })
    } catch {
      // Silently ignore
    }
  }, [])

  const renderPage = () => {
    switch (currentPage) {
      case "transcribe":
        return <TranscribePage />
      case "dictionary":
        return <DictionaryPage />
      case "ocr":
        return <OCRPage onScreenshotTrigger={triggerScreenshot} />
      case "settings":
        return <SettingsPage />
      case "model-settings":
        return <ModelSettingsPage />
      case "about":
        return <AboutPage />
      default:
        return <TranscribePage />
    }
  }

  return (
    <AppProvider>
      <TooltipProvider>
        <SidebarProvider
          open={sidebarOpen}
          onOpenChange={handleSidebarOpenChange}
          style={
            {
              "--sidebar-width": "calc(var(--spacing) * 52)",
              "--header-height": "calc(var(--spacing) * 12)",
            } as React.CSSProperties
          }
        >
          <AppSidebar
            variant="inset"
            currentPage={currentPage}
            onNavigate={setCurrentPage}
          />
          <SidebarInset>
            <SiteHeader currentPage={currentPage} />
            <div className="flex flex-1 flex-col">
              <div className="@container/main flex flex-1 flex-col gap-2">
                <div className="flex flex-col gap-4 py-4 md:gap-6 md:py-6">
                  {renderPage()}
                </div>
              </div>
            </div>
          </SidebarInset>
        </SidebarProvider>
      </TooltipProvider>
    </AppProvider>
  )
}