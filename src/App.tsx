import { useState, useEffect, useCallback } from "react"
import { AppSidebar } from "@/components/app-sidebar"
import {
  SidebarInset,
  SidebarProvider,
} from "@/components/ui/sidebar"
import { TooltipProvider } from "@/components/ui/tooltip"
import { SiteHeader } from "@/components/site-header"
import { TranscribePage } from "@/app/transcribe/page"
import { DictionaryPage } from "@/app/dictionary/page"
import { SettingsPage } from "@/app/settings/page"
import { ModelSettingsPage } from "@/app/settings/model-settings"
import { AboutPage } from "@/app/about/page"
import { AppProvider } from "@/lib/app-context"
import { invoke } from "@tauri-apps/api/core"
import type { AppConfig } from "@/types"

export type Page = "transcribe" | "dictionary" | "settings" | "model-settings" | "about"

export default function App() {
  const [currentPage, setCurrentPage] = useState<Page>("transcribe")
  const [sidebarOpen, setSidebarOpen] = useState(true)

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