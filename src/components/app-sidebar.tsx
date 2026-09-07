import { useState, useEffect, type ComponentProps } from "react"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
} from "@/components/ui/sidebar"
import {
  MicIcon,
  BookOpenIcon,
  CaptionsIcon,
  Settings2Icon,
  CpuIcon,
  InfoIcon,
  CommandIcon,
  SunIcon,
  MoonIcon,
  LanguagesIcon,
} from "lucide-react"
import { getVersion } from "@tauri-apps/api/app"
import { useAppContext } from "@/lib/app-context"
import type { Page } from "@/App"

interface AppSidebarProps extends ComponentProps<typeof Sidebar> {
  currentPage: Page
  onNavigate: (page: Page) => void
}

const labels = {
  zh: {
    features: "功能",
    transcribe: "转录",
    subtitle: "转字幕",
    dictionary: "词典",
    settingsLabel: "设置",
    settings: "设置",
    models: "模型管理",
    about: "关于",
  },
  en: {
    features: "Features",
    transcribe: "Transcribe",
    subtitle: "Subtitles",
    dictionary: "Dictionary",
    settingsLabel: "Settings",
    settings: "Settings",
    models: "Models",
    about: "About",
  },
}

export function AppSidebar({
  currentPage,
  onNavigate,
  ...props
}: AppSidebarProps) {
  const { theme, toggleTheme, language, setLanguage } = useAppContext()
  const l = labels[language]
  const [appVersion, setAppVersion] = useState("")

  useEffect(() => {
    getVersion().then((v) => setAppVersion(v)).catch(() => setAppVersion(""))
  }, [])

  const mainNav = [
    { id: "transcribe" as Page, title: l.transcribe, icon: MicIcon },
    { id: "subtitle" as Page, title: l.subtitle, icon: CaptionsIcon },
  ]

  const settingsNav = [
    { id: "settings" as Page, title: l.settings, icon: Settings2Icon },
    { id: "model-settings" as Page, title: l.models, icon: CpuIcon },
    { id: "dictionary" as Page, title: l.dictionary, icon: BookOpenIcon },
  ]

  return (
    <Sidebar collapsible="offcanvas" {...props}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              asChild
              className="data-[slot=sidebar-menu-button]:p-1.5!"
            >
              <a href="#" onClick={(e) => { e.preventDefault(); onNavigate("transcribe") }}>
                <CommandIcon className="size-5!" />
                <span className="text-base font-semibold">VelociText</span>
              </a>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>{l.features}</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {mainNav.map((item) => (
                <SidebarMenuItem key={item.id}>
                  <SidebarMenuButton
                    tooltip={item.title}
                    isActive={currentPage === item.id}
                    onClick={() => onNavigate(item.id)}
                  >
                    <item.icon />
                    <span>{item.title}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <SidebarGroup className="mt-auto">
          <SidebarGroupLabel>{l.settingsLabel}</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {settingsNav.map((item) => (
                <SidebarMenuItem key={item.id}>
                  <SidebarMenuButton
                    tooltip={item.title}
                    isActive={currentPage === item.id}
                    onClick={() => onNavigate(item.id)}
                  >
                    <item.icon />
                    <span>{item.title}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
              <SidebarMenuItem>
                <SidebarMenuButton
                  tooltip={l.about}
                  isActive={currentPage === "about"}
                  onClick={() => onNavigate("about")}
                >
                  <InfoIcon />
                  <span>{l.about}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <div className="flex items-center justify-between px-3 py-2">
          <span className="text-xs text-muted-foreground">{appVersion ? `v${appVersion}` : ""}</span>
          <div className="flex items-center gap-1">
            <button
              onClick={() => setLanguage(language === "zh" ? "en" : "zh")}
              className="p-1 rounded hover:bg-accent transition-colors"
              title={language === "zh" ? "Switch to English" : "切换到中文"}
            >
              <LanguagesIcon className="size-3.5 text-muted-foreground" />
            </button>
            <button
              onClick={toggleTheme}
              className="p-1 rounded hover:bg-accent transition-colors"
              title={theme === "dark" ? "Switch to light mode" : "切换到深色模式"}
            >
              {theme === "dark" ? (
                <SunIcon className="size-3.5 text-muted-foreground" />
              ) : (
                <MoonIcon className="size-3.5 text-muted-foreground" />
              )}
            </button>
          </div>
        </div>
      </SidebarFooter>
    </Sidebar>
  )
}