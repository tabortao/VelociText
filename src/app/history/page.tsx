import { useState, useEffect } from "react"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { ClockIcon, Trash2Icon, FileTextIcon, ChevronDownIcon, ChevronRightIcon } from "lucide-react"
import { useAppContext } from "@/lib/app-context"
import type { HistoryEntry } from "@/types"

export function HistoryPage() {
  const { t } = useAppContext()
  const [entries, setEntries] = useState<HistoryEntry[]>([])
  const [expandedId, setExpandedId] = useState<string | null>(null)

  const loadHistory = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      const data = await invoke<HistoryEntry[]>("get_history")
      setEntries(data)
    } catch (err) {
      console.error("Failed to load history:", err)
    }
  }

  useEffect(() => {
    loadHistory()
  }, [])

  const handleDelete = async (id: string) => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      await invoke("delete_history", { id })
      setEntries((prev) => prev.filter((e) => e.id !== id))
      if (expandedId === id) setExpandedId(null)
    } catch (err) {
      console.error("Failed to delete history:", err)
    }
  }

  const handleClearAll = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core")
      await invoke("clear_history")
      setEntries([])
      setExpandedId(null)
    } catch (err) {
      console.error("Failed to clear history:", err)
    }
  }

  const formatDate = (iso: string) => {
    const d = new Date(iso)
    return d.toLocaleString()
  }

  return (
    <div className="px-4 lg:px-6 space-y-4">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              <ClockIcon className="size-5" />
              {t("history.title")}
            </CardTitle>
            <CardDescription>{t("history.desc")}</CardDescription>
          </div>
          {entries.length > 0 && (
            <Button variant="ghost" size="sm" onClick={handleClearAll}>
              <Trash2Icon className="size-3.5 mr-1" />
              {t("history.clearAll")}
            </Button>
          )}
        </CardHeader>
        <CardContent>
          {entries.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-center">
              <ClockIcon className="size-12 text-muted-foreground mb-4" />
              <p className="text-muted-foreground font-medium">{t("history.noRecords")}</p>
              <p className="text-sm text-muted-foreground mt-1">{t("history.noRecordsHint")}</p>
            </div>
          ) : (
            <div className="space-y-2">
              {entries.map((entry) => (
                <div key={entry.id}>
                  <div
                    className="flex items-center justify-between p-3 rounded-md border bg-card hover:bg-accent/50 transition-colors cursor-pointer"
                    onClick={() => setExpandedId(expandedId === entry.id ? null : entry.id)}
                  >
                    <div className="flex items-center gap-3 min-w-0">
                      {expandedId === entry.id ? (
                        <ChevronDownIcon className="size-4 text-muted-foreground shrink-0" />
                      ) : (
                        <ChevronRightIcon className="size-4 text-muted-foreground shrink-0" />
                      )}
                      <FileTextIcon className="size-5 text-muted-foreground shrink-0" />
                      <div className="min-w-0">
                        <p className="font-medium text-sm truncate">{entry.fileName}</p>
                        <div className="flex items-center gap-2 mt-0.5">
                          <span className="text-xs text-muted-foreground">
                            {formatDate(entry.createdAt)}
                          </span>
                          {entry.modelType && (
                            <Badge variant="outline" className="text-xs">
                              {entry.modelType}
                            </Badge>
                          )}
                          <span className="text-xs text-muted-foreground">
                            {t("history.segments", { count: String(entry.segmentCount) })}
                          </span>
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center gap-2 shrink-0 ml-4">
                      <span className="text-xs text-muted-foreground">
                        {Math.round(entry.durationSecs)}s
                      </span>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="size-7"
                        onClick={(e) => {
                          e.stopPropagation()
                          handleDelete(entry.id)
                        }}
                      >
                        <Trash2Icon className="size-3.5" />
                      </Button>
                    </div>
                  </div>
                  {expandedId === entry.id && (
                    <div className="mx-2 p-3 border-x border-b rounded-b-md bg-muted/20 text-sm leading-relaxed max-h-[300px] overflow-y-auto">
                      {entry.text ? (
                        <p className="whitespace-pre-wrap">{entry.text}</p>
                      ) : (
                        <span className="text-muted-foreground italic">(No text content)</span>
                      )}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}