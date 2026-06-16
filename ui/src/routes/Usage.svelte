<script lang="ts">
  import { api, type UsageReport, type UsageNode } from "../ipc";
  import { humanBytes } from "../lib/format";
  import Icon from "../lib/Icon.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { onDestroy, onMount } from "svelte";
  import { fly, slide } from "svelte/transition";

  let drives: string[] = [];
  let drive = "C:\\";
  let path = "C:\\";
  let report: UsageReport | null = null;
  let selected = new Set<string>();
  let scanning = false;
  let working = false;
  let status = "";
  let statusKind: "ok" | "warn" | "" = "";
  let progressLabel = "";

  let unlisten: (() => void) | undefined;
  listen<string>("analyze://progress", (e) => {
    progressLabel = e.payload;
  }).then((fn) => (unlisten = fn));
  onDestroy(() => unlisten?.());

  onMount(async () => {
    try {
      drives = await api.listDrives();
      if (drives.length && !drives.includes(drive)) {
        drive = drives[0];
        path = drives[0];
      }
    } catch {
      /* keep default C:\ */
    }
  });

  function crumbs(p: string): { label: string; path: string }[] {
    const parts = p.replace(/\\+$/, "").split("\\").filter(Boolean);
    const out: { label: string; path: string }[] = [];
    let acc = "";
    parts.forEach((seg, idx) => {
      acc = idx === 0 ? seg + "\\" : acc.replace(/\\?$/, "\\") + seg;
      out.push({ label: seg, path: acc });
    });
    return out;
  }

  async function scan(target = path) {
    path = target;
    scanning = true;
    status = "";
    statusKind = "";
    progressLabel = "";
    selected = new Set();
    try {
      report = await api.analyzeDisk(target);
    } catch (e) {
      status = `Ошибка анализа: ${e}`;
      statusKind = "warn";
      report = null;
    } finally {
      scanning = false;
    }
  }

  function openNode(n: UsageNode) {
    if (n.is_dir && n.path) scan(n.path);
    else if (!n.protected && n.path) toggle(n.path);
  }

  function toggle(p: string) {
    selected.has(p) ? selected.delete(p) : selected.add(p);
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

  async function quarantine() {
    const paths = [...selected];
    if (!paths.length) return;
    working = true;
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
        const r = await api.quarantineItems("Анализ диска", v, ps);
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
    working = false;
    await scan(path);
  }

  $: tiles = report ? report.tree.children : [];
  $: maxTile = tiles.reduce((m, t) => Math.max(m, t.size), 1);
  $: sizeOf = (() => {
    const m = new Map<string, number>();
    if (report) {
      for (const n of report.tree.children) if (n.path) m.set(n.path, n.size);
      for (const f of report.largest) m.set(f.path, f.size);
    }
    return m;
  })();
  $: selTotal = [...selected].reduce((s, p) => s + (sizeOf.get(p) ?? 0), 0);
  $: used = report ? Math.max(0, report.volume_total - report.free) : 0;
  $: usedPct = report && report.volume_total ? Math.round((used / report.volume_total) * 100) : 0;
</script>

<header class="bar">
  <span class="hicon"><Icon name="analyze" size={20} /></span>
  <div>
    <h2>Анализ диска</h2>
    <div class="sub faint mono">
      {#if scanning}сканирую…{:else if report}{humanBytes(report.total)} в {report.tree.children.length} элементах{:else}выбери диск и нажми «Сканировать»{/if}
    </div>
  </div>
  <span class="grow"></span>

  <select class="drivesel" bind:value={drive} on:change={() => scan(drive)} disabled={scanning || working}>
    {#each drives as d}<option value={d}>{d}</option>{/each}
    {#if !drives.length}<option value={drive}>{drive}</option>{/if}
  </select>
  <button class="btn btn--ghost btn--sm" on:click={() => scan(drive)} disabled={scanning || working}>
    {#if scanning}<span class="spinner"></span>{:else}<Icon name="refresh" size={16} />{/if}
    Сканировать
  </button>
</header>

{#if report}
  <div class="diskbar" transition:fly={{ y: -6, duration: 180 }}>
    <div class="track"><div class="fill" style="width:{usedPct}%"></div></div>
    <span class="faint mono">занято {humanBytes(used)} из {humanBytes(report.volume_total)} · свободно {humanBytes(report.free)}</span>
  </div>
{/if}

{#if scanning || working}
  <div class="liveband" transition:slide={{ duration: 160 }}>
    <div class="progress"></div>
    <div class="livetext faint mono">
      {#if working}переношу в карантин…{:else if progressLabel}сканирую: {progressLabel}{:else}сканирую…{/if}
    </div>
  </div>
{/if}

{#if status}
  <p class="status" class:warn={statusKind === "warn"} class:ok={statusKind === "ok"} transition:fly={{ y: -4, duration: 160 }}>
    {#if statusKind === "ok"}<Icon name="check" size={15} />{:else if statusKind === "warn"}<Icon name="alert" size={15} />{/if}
    <span>{status}</span>
  </p>
{/if}

{#if report}
  <nav class="crumbs">
    {#each crumbs(path) as c, i (c.path)}
      {#if i > 0}<span class="crsep"><Icon name="chevron" size={12} /></span>{/if}
      <button class="crumb" class:cur={c.path.replace(/\\+$/, "") === path.replace(/\\+$/, "")} on:click={() => scan(c.path)}>{c.label}</button>
    {/each}
    <span class="grow"></span>
    <span class="faint crumbhint">клик по папке — внутрь</span>
  </nav>

  <div class="treemap">
    {#each tiles as t (t.path + t.name)}
      <button
        class="tile"
        class:isdir={t.is_dir}
        class:prot={t.protected}
        class:sel={selected.has(t.path)}
        style="flex-grow:{Math.max(1, Math.round((t.size / maxTile) * 100))}"
        on:click={() => openNode(t)}
        disabled={t.protected && !t.is_dir}
        title={t.path || t.name}
      >
        <span class="tname">
          {#if t.protected}<Icon name="quarantine" size={13} />{:else if t.is_dir}<Icon name="folder" size={13} />{/if}
          <span class="tlabel">{t.name}</span>
        </span>
        <span class="tsize mono">{humanBytes(t.size)}</span>
      </button>
    {/each}
  </div>

  <div class="sechead">
    <span class="eyebrow">самые крупные файлы</span>
    <span class="grow"></span>
    {#if selected.size}
      <span class="faint mono">выбрано {selected.size} · <span class="accent">{humanBytes(selTotal)}</span></span>
    {/if}
  </div>

  <div class="files">
    {#each report.largest as f (f.path)}
      <label class="listrow" class:is-selected={selected.has(f.path)}>
        {#if f.protected}
          <span class="lockcell"><Icon name="quarantine" size={16} /></span>
        {:else}
          <input class="check" type="checkbox" checked={selected.has(f.path)} on:change={() => toggle(f.path)} />
        {/if}
        <span class="path">{f.path}</span>
        <span class="size mono">{humanBytes(f.size)}</span>
      </label>
    {/each}
    {#if !report.largest.length}
      <div class="empty"><div class="ring"><Icon name="analyze" size={28} /></div><div>Крупных файлов не найдено</div></div>
    {/if}
  </div>

  <div class="footer">
    <span class="faint">{#if selected.size}выбрано {selected.size} · {humanBytes(selTotal)}{:else}отметь файлы или плитки для переноса{/if}</span>
    <span class="grow"></span>
    <button class="btn btn--accent" on:click={quarantine} disabled={!selected.size || scanning || working}>
      {#if working}<span class="spinner"></span>{:else}<Icon name="quarantine" size={16} />{/if}
      В карантин · обратимо
    </button>
  </div>
{:else if !scanning}
  <div class="empty">
    <div class="ring"><Icon name="analyze" size={28} /></div>
    <div>Выбери диск и нажми «Сканировать»</div>
    <div class="faint" style="font-size:12.5px">Покажу, что занимает место, и дам убрать лишнее в карантин</div>
  </div>
{/if}

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
  .drivesel {
    font-family: var(--font-mono);
    font-size: 12.5px;
    background: #00000033;
    color: var(--text);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-sm);
    padding: 7px 9px;
    cursor: pointer;
  }
  .drivesel:focus {
    outline: none;
    border-color: var(--acc);
  }
  .diskbar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 14px;
    font-size: 12px;
  }
  .track {
    flex: 1;
    height: 8px;
    border-radius: 999px;
    background: #20232b;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--acc);
    transition: width 0.5s var(--ease, ease);
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
  .crumbs {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 16px;
    flex-wrap: wrap;
  }
  .crsep {
    color: var(--text-faint);
    display: grid;
    place-items: center;
  }
  .crumb {
    border: 0;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    font-family: var(--font-display);
    font-size: 13px;
    padding: 3px 7px;
    border-radius: 6px;
  }
  .crumb:hover {
    color: var(--text);
    background: #ffffff0a;
  }
  .crumb.cur {
    color: var(--acc);
  }
  .crumbhint {
    font-size: 11.5px;
  }
  .treemap {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }
  .tile {
    flex: 1 1 92px;
    min-width: 92px;
    min-height: 66px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 6px;
    text-align: left;
    padding: 9px 11px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--text);
    cursor: default;
    overflow: hidden;
    transition:
      transform 0.12s ease,
      border-color 0.15s ease,
      background 0.15s ease;
  }
  .tile.isdir {
    cursor: pointer;
  }
  .tile.isdir:hover {
    transform: translateY(-1px);
    border-color: var(--line-strong);
    background: #272b34;
  }
  .tile.sel {
    background: var(--acc-soft);
    border-color: rgba(198, 242, 78, 0.4);
    color: var(--acc);
  }
  .tile.prot {
    background: #15161b;
    border-style: dashed;
    color: var(--text-faint);
    cursor: not-allowed;
  }
  .tname {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-display);
    font-size: 12.5px;
    min-width: 0;
  }
  .tlabel {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tsize {
    font-size: 11.5px;
    color: var(--text-dim);
  }
  .tile.sel .tsize {
    color: var(--acc);
  }
  .sechead {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 22px 0 6px;
  }
  .accent {
    color: var(--acc);
  }
  .files {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .lockcell {
    width: 19px;
    display: grid;
    place-items: center;
    color: var(--text-faint);
    flex: 0 0 auto;
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
  .footer {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 18px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
    font-size: 12.5px;
  }
</style>
