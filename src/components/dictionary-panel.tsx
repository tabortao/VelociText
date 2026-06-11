import { useState, useEffect, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import { PlusIcon, Trash2Icon, SaveIcon, Loader2Icon } from "lucide-react"
import { useAppContext } from "@/lib/app-context"
import type { HotwordEntry, ReplacementEntry, DictionaryConfig } from "@/types"

interface DictionaryPanelProps {
  onRebuildingChange?: (rebuilding: boolean) => void
}

export function DictionaryPanel({ onRebuildingChange }: DictionaryPanelProps) {
  const { t } = useAppContext()
  const [hotwords, setHotwords] = useState<HotwordEntry[]>([])
  const [replacements, setReplacements] = useState<ReplacementEntry[]>([])
  const [savingHotwords, setSavingHotwords] = useState(false)
  const [savingReplacements, setSavingReplacements] = useState(false)
  const [rebuilding, setRebuilding] = useState(false)
  const [statusMsg, setStatusMsg] = useState("")
  const statusTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Form inputs
  const [newWord, setNewWord] = useState("")
  const [newWeight, setNewWeight] = useState("1.5")
  const [newOriginal, setNewOriginal] = useState("")
  const [newReplacement, setNewReplacement] = useState("")

  // Flash status message
  const showStatus = (msg: string, durationMs = 2500) => {
    if (statusTimerRef.current) clearTimeout(statusTimerRef.current)
    setStatusMsg(msg)
    statusTimerRef.current = setTimeout(() => setStatusMsg(""), durationMs)
  }

  // Cleanup timer on unmount
  useEffect(() => {
    return () => {
      if (statusTimerRef.current) clearTimeout(statusTimerRef.current)
    }
  }, [])

  // Load config on mount
  useEffect(() => {
    const load = async () => {
      try {
        const config = await invoke<DictionaryConfig>("get_dictionary_config")
        setHotwords(config.hotwords)
        setReplacements(config.replacements)
      } catch (err) {
        console.error("Failed to load dictionary config:", err)
      }
    }
    load()
  }, [])

  // ── Hotwords ──────────────────────────────────────────────────────────
  const handleAddHotword = () => {
    const word = newWord.trim()
    if (!word) return
    const weight = parseFloat(newWeight)
    if (isNaN(weight) || weight < 0 || weight > 10) return

    setHotwords((prev) => [...prev, { word, weight }])
    setNewWord("")
    setNewWeight("1.5")
  }

  const handleRemoveHotword = (idx: number) => {
    setHotwords((prev) => prev.filter((_, i) => i !== idx))
  }

  const handleSaveHotwords = async () => {
    setSavingHotwords(true)
    setRebuilding(true)
    onRebuildingChange?.(true)
    try {
      await invoke("save_hotwords", { hotwords })
      showStatus(hotwords.length > 0 ? t("dictionary.hotwordSaved") : t("dictionary.replacementSaved"), 4000)
      // Reload config from backend to sync
      const config = await invoke<DictionaryConfig>("get_dictionary_config")
      setHotwords(config.hotwords)
    } catch (err) {
      showStatus(String(err), 6000)
      // Reload config from backend (rollback may have occurred)
      try {
        const config = await invoke<DictionaryConfig>("get_dictionary_config")
        setHotwords(config.hotwords)
      } catch {}
    } finally {
      setSavingHotwords(false)
      setRebuilding(false)
      onRebuildingChange?.(false)
    }
  }

  const handleHotwordKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleAddHotword()
  }

  // ── Replacements ──────────────────────────────────────────────────────
  const handleAddReplacement = () => {
    const original = newOriginal.trim()
    const replacement = newReplacement.trim()
    if (!original || !replacement) return

    setReplacements((prev) => [...prev, { original, replacement }])
    setNewOriginal("")
    setNewReplacement("")
  }

  const handleRemoveReplacement = (idx: number) => {
    setReplacements((prev) => prev.filter((_, i) => i !== idx))
  }

  const handleSaveReplacements = async () => {
    setSavingReplacements(true)
    try {
      await invoke("save_replacements", { replacements })
      showStatus(t("dictionary.replacementSaved"), 2500)
      // Reload from backend to sync
      const config = await invoke<DictionaryConfig>("get_dictionary_config")
      setReplacements(config.replacements)
    } catch (err) {
      showStatus(String(err), 6000)
      try {
        const config = await invoke<DictionaryConfig>("get_dictionary_config")
        setReplacements(config.replacements)
      } catch {}
    } finally {
      setSavingReplacements(false)
    }
  }

  const handleReplacementKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleAddReplacement()
  }

  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="text-base">{t("dictionary.title")}</CardTitle>
            <CardDescription>{t("dictionary.desc")}</CardDescription>
          </div>
          {statusMsg && (
            <Badge variant="secondary" className="text-xs">
              {statusMsg}
            </Badge>
          )}
        </div>
      </CardHeader>
      <CardContent>
        <Tabs defaultValue="hotwords">
          <TabsList className="mb-3">
            <TabsTrigger value="hotwords">{t("dictionary.hotwords")}</TabsTrigger>
            <TabsTrigger value="replacements">{t("dictionary.replacements")}</TabsTrigger>
          </TabsList>

          {/* ── Hotwords Tab ── */}
          <TabsContent value="hotwords" className="space-y-3">
            <p className="text-xs text-muted-foreground">{t("dictionary.hotwordsDesc")}</p>

            {/* Add form */}
            <div className="flex items-end gap-2">
              <div className="flex-1 space-y-1">
                <Label className="text-xs">{t("dictionary.word")}</Label>
                <Input
                  placeholder={t("dictionary.wordPlaceholder")}
                  value={newWord}
                  onChange={(e) => setNewWord(e.target.value)}
                  onKeyDown={handleHotwordKeyDown}
                  className="h-8 text-sm"
                />
              </div>
              <div className="w-24 space-y-1">
                <Label className="text-xs">{t("dictionary.weight")}</Label>
                <Input
                  type="number"
                  min={0}
                  max={10}
                  step={0.1}
                  value={newWeight}
                  onChange={(e) => setNewWeight(e.target.value)}
                  onKeyDown={handleHotwordKeyDown}
                  className="h-8 text-sm"
                  title={t("dictionary.weightHint")}
                />
              </div>
              <Button
                size="sm"
                variant="outline"
                onClick={handleAddHotword}
                disabled={!newWord.trim()}
                className="h-8"
              >
                <PlusIcon className="size-3.5 mr-1" />
                {t("dictionary.add")}
              </Button>
            </div>

            {/* Hotwords table */}
            {hotwords.length > 0 ? (
              <div className="border rounded-md overflow-hidden max-h-[200px] overflow-y-auto">
                <table className="w-full text-sm">
                  <thead className="bg-muted/50 sticky top-0">
                    <tr>
                      <th className="text-left px-3 py-1.5">{t("dictionary.word")}</th>
                      <th className="text-left px-3 py-1.5 w-[80px]">{t("dictionary.weight")}</th>
                      <th className="text-right px-3 py-1.5 w-[50px]"></th>
                    </tr>
                  </thead>
                  <tbody>
                    {hotwords.map((hw, i) => (
                      <tr key={i} className="border-t border-border hover:bg-accent/50">
                        <td className="px-3 py-1.5">{hw.word}</td>
                        <td className="px-3 py-1.5 text-muted-foreground tabular-nums">
                          {hw.weight}
                        </td>
                        <td className="px-3 py-1.5 text-right">
                          <button
                            className="p-1 rounded hover:bg-muted transition-colors text-muted-foreground hover:text-destructive"
                            title={t("dictionary.remove")}
                            onClick={() => handleRemoveHotword(i)}
                          >
                            <Trash2Icon className="size-3.5" />
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground text-center py-4">
                {t("dictionary.emptyHotwords")}
              </p>
            )}

            {/* Save button */}
            <div className="flex justify-end">
              <Button
                size="sm"
                onClick={handleSaveHotwords}
                disabled={savingHotwords}
              >
                {savingHotwords || rebuilding ? (
                  <Loader2Icon className="size-3.5 mr-1 animate-spin" />
                ) : (
                  <SaveIcon className="size-3.5 mr-1" />
                )}
                {savingHotwords || rebuilding
                  ? t("dictionary.rebuilding")
                  : t("dictionary.save")}
              </Button>
            </div>
          </TabsContent>

          {/* ── Replacements Tab ── */}
          <TabsContent value="replacements" className="space-y-3">
            <p className="text-xs text-muted-foreground">{t("dictionary.replacementsDesc")}</p>

            {/* Add form */}
            <div className="flex items-end gap-2">
              <div className="flex-1 space-y-1">
                <Label className="text-xs">{t("dictionary.original")}</Label>
                <Input
                  placeholder={t("dictionary.originalPlaceholder")}
                  value={newOriginal}
                  onChange={(e) => setNewOriginal(e.target.value)}
                  onKeyDown={handleReplacementKeyDown}
                  className="h-8 text-sm"
                />
              </div>
              <div className="flex-1 space-y-1">
                <Label className="text-xs">{t("dictionary.replacement")}</Label>
                <Input
                  placeholder={t("dictionary.replacementPlaceholder")}
                  value={newReplacement}
                  onChange={(e) => setNewReplacement(e.target.value)}
                  onKeyDown={handleReplacementKeyDown}
                  className="h-8 text-sm"
                />
              </div>
              <Button
                size="sm"
                variant="outline"
                onClick={handleAddReplacement}
                disabled={!newOriginal.trim() || !newReplacement.trim()}
                className="h-8"
              >
                <PlusIcon className="size-3.5 mr-1" />
                {t("dictionary.add")}
              </Button>
            </div>

            {/* Replacements table */}
            {replacements.length > 0 ? (
              <div className="border rounded-md overflow-hidden max-h-[200px] overflow-y-auto">
                <table className="w-full text-sm">
                  <thead className="bg-muted/50 sticky top-0">
                    <tr>
                      <th className="text-left px-3 py-1.5">{t("dictionary.original")}</th>
                      <th className="text-left px-3 py-1.5">{t("dictionary.replacement")}</th>
                      <th className="text-right px-3 py-1.5 w-[50px]"></th>
                    </tr>
                  </thead>
                  <tbody>
                    {replacements.map((r, i) => (
                      <tr key={i} className="border-t border-border hover:bg-accent/50">
                        <td className="px-3 py-1.5 text-muted-foreground">{r.original}</td>
                        <td className="px-3 py-1.5">{r.replacement}</td>
                        <td className="px-3 py-1.5 text-right">
                          <button
                            className="p-1 rounded hover:bg-muted transition-colors text-muted-foreground hover:text-destructive"
                            title={t("dictionary.remove")}
                            onClick={() => handleRemoveReplacement(i)}
                          >
                            <Trash2Icon className="size-3.5" />
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <p className="text-xs text-muted-foreground text-center py-4">
                {t("dictionary.emptyReplacements")}
              </p>
            )}

            {/* Save button */}
            <div className="flex justify-end">
              <Button
                size="sm"
                onClick={handleSaveReplacements}
                disabled={savingReplacements}
              >
                {savingReplacements ? (
                  <Loader2Icon className="size-3.5 mr-1 animate-spin" />
                ) : (
                  <SaveIcon className="size-3.5 mr-1" />
                )}
                {savingReplacements ? t("dictionary.saving") : t("dictionary.save")}
              </Button>
            </div>
          </TabsContent>
        </Tabs>
      </CardContent>
    </Card>
  )
}