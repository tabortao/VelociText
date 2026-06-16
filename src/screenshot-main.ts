/// Screenshot selection window — runs in a separate transparent fullscreen Tauri window.
/// References snow-shot's draw page architecture: transparent window covering all monitors,
/// screenshot displayed at 1:1 scale, mouse coordinates map directly to image pixels.
///
/// Communication:
/// - Uses `invoke("get_screenshot_data")` to get screenshot data (avoids event timing issues)
/// - Uses `invoke` commands to: perform OCR, copy to clipboard, close window
/// - No dependency on @tauri-apps/api npm packages (uses withGlobalTauri)

// Type declarations for withGlobalTauri
declare global {
  interface Window {
    __TAURI__: {
      core: {
        invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
        convertFileSrc: (path: string) => string;
      };
    };
  }
}

const img = document.getElementById("screenshot-img") as HTMLImageElement;
const selection = document.getElementById("selection") as HTMLDivElement;
const hint = document.getElementById("hint") as HTMLDivElement;
const processing = document.getElementById("processing") as HTMLDivElement;

let selecting = false;
let startX = 0;
let startY = 0;
let imagePath = "";
let modelVersion = "";
let isProcessing = false;

const { invoke, convertFileSrc } = window.__TAURI__.core;

// On load, fetch screenshot data from Rust backend via invoke (no timing issues)
(async () => {
  try {
    const data = (await invoke("get_screenshot_data")) as {
      imagePath: string;
      width: number;
      height: number;
      modelVersion: string;
    };
    imagePath = data.imagePath;
    modelVersion = data.modelVersion;
    const fileUrl = convertFileSrc(data.imagePath);
    img.src = fileUrl;
    img.style.width = data.width + "px";
    img.style.height = data.height + "px";
  } catch (err) {
    console.error("Failed to get screenshot data:", err);
  }
})();

// Mouse events for region selection
document.addEventListener("mousedown", (e: MouseEvent) => {
  if (isProcessing) return;
  if (e.button !== 0) return; // left click only
  selecting = true;
  startX = e.clientX;
  startY = e.clientY;
  selection.style.display = "none";
  selection.style.left = startX + "px";
  selection.style.top = startY + "px";
  selection.style.width = "0px";
  selection.style.height = "0px";
});

document.addEventListener("mousemove", (e: MouseEvent) => {
  if (!selecting || isProcessing) return;
  const x = Math.min(startX, e.clientX);
  const y = Math.min(startY, e.clientY);
  const w = Math.abs(e.clientX - startX);
  const h = Math.abs(e.clientY - startY);
  selection.style.display = "block";
  selection.style.left = x + "px";
  selection.style.top = y + "px";
  selection.style.width = w + "px";
  selection.style.height = h + "px";
});

document.addEventListener("mouseup", async (e: MouseEvent) => {
  if (!selecting || isProcessing) return;
  selecting = false;

  const x = Math.min(startX, e.clientX);
  const y = Math.min(startY, e.clientY);
  const w = Math.abs(e.clientX - startX);
  const h = Math.abs(e.clientY - startY);

  // Minimum selection size
  if (w < 10 || h < 10) {
    selection.style.display = "none";
    return;
  }

  isProcessing = true;
  hint.style.display = "none";
  processing.style.display = "block";

  try {
    // Perform OCR on the selected region via Rust command
    const result = (await invoke("ocr_screenshot_region", {
      imagePath,
      x: Math.round(x),
      y: Math.round(y),
      width: Math.round(w),
      height: Math.round(h),
      modelVersion,
    })) as {
      ocrResult: {
        textBlocks: Array<{ text: string; confidence: number; boxPoints: unknown }>;
        totalTimeMs: number;
      };
      croppedImagePath: string;
    };

    const text = result.ocrResult.textBlocks.map((b: { text: string }) => b.text).join("\n");

    // Copy to clipboard via Rust command
    if (text) {
      await invoke("copy_text_to_clipboard", { text });
    }

    // Notify main window about the result (including cropped image path and full OCR result)
    await invoke("screenshot_ocr_done", {
      text,
      timeMs: result.ocrResult.totalTimeMs,
      croppedImagePath: result.croppedImagePath,
      ocrResult: result.ocrResult,
    });
  } catch (err) {
    console.error("Screenshot OCR failed:", err);
  }

  // Close this screenshot window via Rust command
  try {
    await invoke("close_screenshot_window");
  } catch (err) {
    console.error("Failed to close screenshot window:", err);
  }
});

// ESC to cancel
document.addEventListener("keydown", (e: KeyboardEvent) => {
  if (e.key === "Escape" && !isProcessing) {
    invoke("close_screenshot_window").catch((err: unknown) => {
      console.error("Failed to close screenshot window:", err);
    });
  }
});
