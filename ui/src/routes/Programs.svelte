<script lang="ts">
  import { api, type Program } from "../ipc";
  import { humanBytes } from "../lib/format";
  import Icon from "../lib/Icon.svelte";
  import { fly } from "svelte/transition";
  import { tweened } from "svelte/motion";
  import { cubicOut } from "svelte/easing";

  let progs: Program[] = [];
  let selected = new Set<string>();
  let loading = false;
  let working = false;
  let status = "";

  const key = (p: Program) => `${p.name}|${p.publisher ?? ""}`;

  const selT = tweened(0, { duration: 420, easing: cubicOut });
  $: selectedTotal = progs
    .filter((p) => selected.has(key(p)))
    .reduce((s, p) => s + p.real_size, 0);
  $: selT.set(selectedTotal);
  $: totalAll = progs.reduce((s, p) => s + p.real_size, 0);

  async function load() {
    loading = true;
    status = "";
    try {
      progs = await api.getPrograms();
      progs.sort((a, b) => b.real_size - a.real_size);
    } catch (e) {
      status = `Не удалось получить список программ: ${e}`;
    } finally {
      loading = false;
    }
  }

  function toggle(k: string) {
    selected.has(k) ? selected.delete(k) : selected.add(k);
    selected = selected;
  }

  async function uninstall() {
    const chosen = progs.filter((p) => selected.has(key(p)));
    if (!chosen.length) return;
    working = true;
    status = `Удаляю ${chosen.length}…`;
    try {
      const results = await api.uninstallPrograms(chosen);
      const failed = results.filter((r) => !r.ok);
      status = failed.length
        ? `Готово, но не удалось: ${failed.map((f) => `${f.program} (${f.message})`).join("; ")}`
        : `Удалено программ: ${chosen.length}`;
    } catch (e) {
      status = `Ошибка удаления: ${e}`;
    }
    working = false;
    selected = new Set();
    await load();
  }

  load();
</script>

<header class="bar">
  <span class="hicon"><Icon name="programs" size={20} /></span>
  <div>
    <h2>Программы</h2>
    <div class="sub faint mono">
      {#if loading}считаю размеры…{:else}{progs.length} установлено · {humanBytes(totalAll)}{/if}
    </div>
  </div>
  <span class="grow"></span>

  {#if selected.size}
    <div class="seltotal" transition:fly={{ y: -6, duration: 180 }}>
      <span class="eyebrow">выбрано</span>
      <span class="mono big">{humanBytes($selT)}</span>
    </div>
  {/if}

  <button class="btn btn--ghost btn--sm" on:click={load} disabled={loading || working}>
    {#if loading}<span class="spinner"></span>{:else}<Icon name="refresh" size={16} />{/if}
    Обновить
  </button>
  <button class="btn btn--danger" on:click={uninstall} disabled={!selected.size || working}>
    {#if working}<span class="spinner"></span>{:else}<Icon name="trash" size={16} />{/if}
    Удалить ({selected.size})
  </button>
</header>

{#if status}
  <p class="status" transition:fly={{ y: -4, duration: 160 }}>{status}</p>
{/if}

<div class="list">
  {#if loading}
    {#each Array(9) as _, i}
      <div class="listrow" style="opacity:{1 - i * 0.08}">
        <div class="skel" style="width:19px;height:19px;border-radius:6px"></div>
        <div style="flex:1">
          <div class="skel" style="height:13px;width:{42 + ((i * 13) % 38)}%"></div>
          <div class="skel" style="height:9px;width:90px;margin-top:7px"></div>
        </div>
        <div class="skel" style="height:12px;width:64px"></div>
      </div>
    {/each}
  {:else if progs.length === 0}
    <div class="empty">
      <div class="ring"><Icon name="programs" size={26} /></div>
      <div>Список пуст</div>
    </div>
  {:else}
    {#each progs as p, i (key(p))}
      <label
        class="listrow"
        class:is-selected={selected.has(key(p))}
        in:fly={{ y: 8, duration: 220, delay: Math.min(i * 11, 260) }}
      >
        <input
          class="check"
          type="checkbox"
          checked={selected.has(key(p))}
          on:change={() => toggle(key(p))}
        />
        <span class="name">
          {p.name}
          <span class="pub faint">{p.publisher ?? "—"}</span>
        </span>
        <span class="size mono" class:est={p.estimated} title={p.estimated ? "оценка из реестра" : "реальный размер на диске"}>
          {#if p.real_size}{p.estimated ? "~" : ""}{humanBytes(p.real_size)}{:else}—{/if}
        </span>
      </label>
    {/each}
  {/if}
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
  .status {
    font-size: 13px;
    color: var(--text-dim);
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    padding: 9px 12px;
    margin: 12px 0 0;
  }
  .list {
    margin-top: 10px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .pub {
    font-size: 11.5px;
  }
  .size {
    font-size: 13px;
    color: var(--text);
    white-space: nowrap;
  }
  .size.est {
    color: var(--text-dim);
  }
</style>
