<script lang="ts">
  import { api, type Category, type CategoryId } from "../ipc";
  import { humanBytes } from "../lib/format";
  import Icon from "../lib/Icon.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { onDestroy } from "svelte";
  import { fly, slide } from "svelte/transition";
  import { tweened } from "svelte/motion";
  import { cubicOut } from "svelte/easing";

  const DEFAULT: CategoryId[] = ["SystemTemp", "BrowserCache", "DevCache"];
  let cats: Category[] = [];
  let selected = new Set<string>();
  let scanning = false;
  let cleaning = false;
  let scanned = false;
  let progressLabel = "";
  let status = "";
  let statusKind: "ok" | "warn" | "" = "";

  let unlisten: (() => void) | undefined;
  listen<string>("scan://progress", (e) => {
    progressLabel = e.payload;
  }).then((fn) => (unlisten = fn));
  onDestroy(() => unlisten?.());

  const selT = tweened(0, { duration: 420, easing: cubicOut });
  $: total = cats
    .flatMap((c) => c.items)
    .filter((i) => selected.has(i.path))
    .reduce((s, i) => s + i.size, 0);
  $: selT.set(total);

  async function scan() {
    scanning = true;
    status = "";
    statusKind = "";
    progressLabel = "";
    try {
      cats = await api.scan(DEFAULT);
      selected = new Set(cats.flatMap((c) => c.items.map((i) => i.path)));
      scanned = true;
    } catch (e) {
      status = `Ошибка сканирования: ${e}`;
      statusKind = "warn";
    } finally {
      scanning = false;
    }
  }

  function toggle(path: string) {
    selected.has(path) ? selected.delete(path) : selected.add(path);
    selected = selected;
  }

  function volumeRoot(p: string): string {
    if (p.startsWith("\\\\")) {
      const third = p.indexOf("\\", 2);
      const fourth = third >= 0 ? p.indexOf("\\", third + 1) : -1;
      return fourth >= 0 ? p.slice(0, fourth + 1) : p;
    }
    return p.slice(0, 2) + "\\";
  }

  async function clean() {
    const paths = [...selected];
    if (!paths.length) return;
    cleaning = true;
    status = "";
    statusKind = "";
    const byVol = new Map<string, string[]>();
    for (const p of paths) {
      const v = volumeRoot(p);
      byVol.set(v, [...(byVol.get(v) ?? []), p]);
    }
    let freed = 0;
    let skipped = 0;
    const errors: string[] = [];
    for (const [v, ps] of byVol) {
      try {
        const r = await api.quarantineItems("Очистка", v, ps);
        freed += r.freed;
        skipped += r.skipped.length;
        if (!r.ok) errors.push(`${v} — ${r.message}`);
      } catch (e) {
        errors.push(`${v} — ${e}`);
      }
    }
    const parts = [`Освобождено ${humanBytes(freed)}`];
    if (skipped) parts.push(`пропущено ${skipped} (заняты или недоступны)`);
    if (errors.length) parts.push(`ошибки: ${errors.join("; ")}`);
    else if (!skipped) parts[0] = `Освобождено ${humanBytes(freed)} — всё в карантине, можно вернуть`;
    status = parts.join(" · ");
    statusKind = errors.length ? "warn" : "ok";
    cleaning = false;
    await scan();
  }

  $: catSize = (c: Category) => c.items.reduce((s, i) => s + i.size, 0);
</script>

<header class="bar">
  <span class="hicon"><Icon name="clean" size={20} /></span>
  <div>
    <h2>Очистка</h2>
    <div class="sub faint mono">
      {#if scanning}сканирую…{:else if scanned}{cats.flatMap((c) => c.items).length} объектов найдено{:else}готов к сканированию{/if}
    </div>
  </div>
  <span class="grow"></span>

  {#if scanned && !scanning}
    <div class="seltotal" transition:fly={{ y: -6, duration: 180 }}>
      <span class="eyebrow">к очистке</span>
      <span class="mono big">{humanBytes($selT)}</span>
    </div>
  {/if}

  <button class="btn btn--ghost btn--sm" on:click={scan} disabled={scanning || cleaning}>
    {#if scanning}<span class="spinner"></span>{:else}<Icon name="search" size={16} />{/if}
    Сканировать
  </button>
  <button class="btn btn--accent" on:click={clean} disabled={!selected.size || scanning || cleaning}>
    {#if cleaning}<span class="spinner"></span>{:else}<Icon name="clean" size={16} />{/if}
    В карантин
  </button>
</header>

{#if scanning || cleaning}
  <div class="liveband" transition:slide={{ duration: 160 }}>
    <div class="progress"></div>
    <div class="livetext faint mono">
      {#if cleaning}переношу в карантин…{:else if progressLabel}сканирую: {progressLabel}{:else}сканирую…{/if}
    </div>
  </div>
{/if}

{#if status}
  <p class="status" class:warn={statusKind === "warn"} class:ok={statusKind === "ok"} transition:fly={{ y: -4, duration: 160 }}>
    {#if statusKind === "ok"}<Icon name="check" size={15} />{:else if statusKind === "warn"}<Icon name="alert" size={15} />{/if}
    <span>{status}</span>
  </p>
{/if}

<div class="content">
  {#if !scanned && !scanning}
    <div class="empty">
      <div class="ring"><Icon name="clean" size={28} /></div>
      <div>Нажми «Сканировать», чтобы найти мусор</div>
      <div class="faint" style="font-size:12.5px">Временные файлы, кэш браузеров и сборок</div>
    </div>
  {/if}

  {#each cats as c, ci (c.id)}
    {#if c.items.length}
      <section in:fly={{ y: 10, duration: 240, delay: ci * 60 }}>
        <div class="sechead">
          <span class="chip chip--acc">{c.label}</span>
          <span class="faint mono">{humanBytes(catSize(c))}</span>
          <span class="faint">· {c.items.length}</span>
        </div>
        {#each c.items as i (i.path)}
          <label class="listrow" class:is-selected={selected.has(i.path)}>
            <input class="check" type="checkbox" checked={selected.has(i.path)} on:change={() => toggle(i.path)} />
            <span class="path">{i.path}</span>
            <span class="size mono">{humanBytes(i.size)}</span>
          </label>
        {/each}
      </section>
    {/if}
  {/each}
</div>

<style>
  .hicon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 11px;
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--acc);
    flex: 0 0 auto;
  }
  .sub {
    font-size: 12px;
    margin-top: 2px;
  }
  .seltotal {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    line-height: 1.1;
  }
  .seltotal .big {
    font-size: 17px;
    color: var(--acc);
  }
  .liveband {
    margin-top: 14px;
  }
  .livetext {
    font-size: 12px;
    margin-top: 8px;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 13px;
    border-radius: var(--r-sm);
    padding: 10px 13px;
    margin: 14px 0 0;
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--text-dim);
  }
  .status.ok {
    color: var(--acc);
    background: var(--acc-soft);
    border-color: rgba(198, 242, 78, 0.3);
  }
  .status.warn {
    color: var(--danger);
    background: var(--danger-soft);
    border-color: rgba(255, 111, 97, 0.35);
  }
  .content {
    margin-top: 16px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .sechead {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
  }
  .path {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    word-break: break-all;
    color: var(--text-dim);
  }
  .size {
    font-size: 12.5px;
    white-space: nowrap;
  }
</style>
