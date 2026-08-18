<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  interface GradeSettings {
    invert: boolean;
    mode: number;
    base_density: [number, number, number];
    exposure: [number, number, number];
    d_min: [number, number, number];
    d_max: [number, number, number];
    gamma: number;
    highlights: number;
    shadows: number;
    saturation: number;
    temperature: number;
    tint: number;
  }

  let { imageId }: { imageId: number | null } = $props();

  function defaults(): GradeSettings {
    return {
      invert: false,
      mode: 0,
      base_density: [0, 0, 0],
      exposure: [0, 0, 0],
      d_min: [0, 0, 0],
      d_max: [2, 2, 2],
      gamma: 1,
      highlights: 0,
      shadows: 0,
      saturation: 0,
      temperature: 0,
      tint: 0,
    };
  }

  let settings = $state<GradeSettings>(defaults());
  let preview = $state<string | null>(null);
  let busy = $state(false);
  let previewError = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function refresh() {
    if (imageId == null) return;
    busy = true;
    previewError = null;
    invoke<string | null>("grade_preview", { id: imageId, settings, maxEdge: 1024 })
      .then((p) => (preview = p))
      .catch((e) => (previewError = String(e)))
      .finally(() => (busy = false));
  }

  function scheduleRefresh() {
    clearTimeout(timer);
    timer = setTimeout(refresh, 250);
  }

  async function autoInvert() {
    if (imageId == null) return;
    try {
      settings = await invoke<GradeSettings>("grade_analyze", {
        id: imageId,
        lowQuantile: 0.01,
        highQuantile: 0.98,
      });
      refresh();
    } catch (e) {
      previewError = String(e);
    }
  }

  // Refresh whenever the frame changes or settings change.
  $effect(() => {
    void imageId;
    refresh();
    return () => clearTimeout(timer);
  });

  // Per-channel triple slider row.
  function channelRow(
    label: string,
    field: "base_density" | "exposure" | "d_min" | "d_max",
    min: number,
    max: number,
    step: number,
  ) {
    return { label, field, min, max, step };
  }
  const channelRows = $derived([
    channelRow("Base density", "base_density", -1, 4, 0.01),
    channelRow("Exposure", "exposure", -2, 2, 0.01),
    channelRow("D-Min", "d_min", -1, 4, 0.01),
    channelRow("D-Max", "d_max", -1, 4, 0.01),
  ]);
</script>

{#if imageId == null}
  <div class="grade-empty">
    <p class="hint">打开一张胶片扫描后即可校色（负片反相、密度、曝光、Gamma、白平衡…）</p>
  </div>
{:else}
  <div class="grade-workspace">
    <div class="grade-panel">
      <div class="grade-head">
        <button class="btn btn-primary" onclick={autoInvert} title="自动分析片基密度与 D-Min/D-Max 并反相">
          Auto Invert
        </button>
        <label class="grade-toggle">
          <input type="checkbox" bind:checked={settings.invert} onchange={refresh} />
          反相 (Invert)
        </label>
        <label class="grade-toggle">
          <select bind:value={settings.mode} onchange={refresh}>
            <option value={0}>Color</option>
            <option value={1}>B&W</option>
          </select>
          模式
        </label>
      </div>

      <div class="grade-row">
        <span class="grade-label">Gamma</span>
        <input type="range" min="0.2" max="3" step="0.05" bind:value={settings.gamma} oninput={scheduleRefresh} />
        <span class="grade-value">{settings.gamma.toFixed(2)}</span>
      </div>

      {#each [
        { label: "Highlights", key: "highlights" },
        { label: "Shadows", key: "shadows" },
        { label: "Saturation", key: "saturation" },
        { label: "Temperature", key: "temperature" },
        { label: "Tint", key: "tint" },
      ] as r}
        <div class="grade-row">
          <span class="grade-label">{r.label}</span>
          <input
            type="range"
            min="-1"
            max="1"
            step="0.01"
            bind:value={settings[r.key as keyof GradeSettings] as number}
            oninput={scheduleRefresh}
          />
          <span class="grade-value">{Number(settings[r.key as keyof GradeSettings]).toFixed(2)}</span>
        </div>
      {/each}

      {#each channelRows as row}
        <div class="grade-row">
          <span class="grade-label">{row.label} R/G/B</span>
          {#each [0, 1, 2] as c}
            <input
              type="range"
              min={row.min}
              max={row.max}
              step={row.step}
              bind:value={settings[row.field][c]}
              oninput={scheduleRefresh}
            />
          {/each}
        </div>
      {/each}

      {#if previewError}
        <p class="hint" style="color: var(--detect)">校色失败：{previewError}</p>
      {/if}
    </div>

    <div class="grade-preview">
      <div class="grade-preview-title">
        <span>校色预览</span>
        {#if busy}<span class="hint">更新中…</span>{/if}
      </div>
      {#if preview}
        <img src={`data:image/png;base64,${preview}`} alt="校色预览" />
      {:else}
        <p class="hint">暂无预览</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .grade-workspace {
    display: flex;
    gap: var(--space-3);
    height: 100%;
    padding: var(--space-3);
    box-sizing: border-box;
    overflow: hidden;
  }
  .grade-panel {
    width: 300px;
    flex: none;
    overflow-y: auto;
    padding: var(--space-2);
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-1);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .grade-head {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    flex-wrap: wrap;
  }
  .grade-toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--text-sm);
    color: var(--text-2);
  }
  .grade-row {
    display: grid;
    grid-template-columns: 90px 1fr 44px;
    align-items: center;
    gap: var(--space-1);
  }
  .grade-label {
    font-size: var(--text-sm);
    color: var(--text-2);
    white-space: nowrap;
  }
  .grade-value {
    font-size: var(--text-sm);
    color: var(--text-2);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .grade-row input[type="range"] {
    width: 100%;
  }
  .grade-preview {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: var(--radius-1);
    background: var(--bg-2);
    overflow: hidden;
  }
  .grade-preview-title {
    padding: var(--space-2);
    display: flex;
    gap: var(--space-2);
    align-items: center;
    border-bottom: 1px solid var(--border);
    font-size: var(--text-sm);
    color: var(--text-2);
  }
  .grade-preview img {
    width: 100%;
    height: auto;
    object-fit: contain;
    flex: 1;
  }
  .grade-empty {
    display: grid;
    place-items: center;
    height: 100%;
  }
</style>
