import { useState, useEffect } from "react"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { InfoIcon } from "lucide-react"
import { getVersion } from "@tauri-apps/api/app"
import { useAppContext } from "@/lib/app-context"

function GithubIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor">
      <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z" />
    </svg>
  )
}

export function AboutPage() {
  const { t } = useAppContext()
  const [appVersion, setAppVersion] = useState("")

  useEffect(() => {
    getVersion().then((v) => setAppVersion(v)).catch(() => setAppVersion(""))
  }, [])

  return (
    <div className="px-4 lg:px-6 space-y-4">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <InfoIcon className="size-5" />
            VelociText
          </CardTitle>
          <CardDescription>{t("about.desc")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            {t("about.description")}
          </p>

          <div className="space-y-2">
            <h3 className="text-sm font-medium">{t("about.techStack")}</h3>
            <ul className="list-disc list-inside text-sm text-muted-foreground space-y-1">
              <li>Tauri v2 + React 19 + TypeScript + Tailwind CSS v4</li>
              <li>sherpa-onnx (SenseVoice-Small) for ASR</li>
              <li>symphonia for pure Rust audio/video decoding</li>
              <li>shadcn/ui components</li>
            </ul>
          </div>

          <div className="space-y-2">
            <h3 className="text-sm font-medium">{t("about.version")}</h3>
            <p className="text-sm text-muted-foreground">{appVersion ? `v${appVersion}` : ""}</p>
          </div>

          <a
            href="https://github.com/tabortao/VelociText"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-2 text-sm text-primary hover:underline"
          >
            <GithubIcon className="size-4" />
            github.com/tabortao/VelociText
          </a>
        </CardContent>
      </Card>
    </div>
  )
}