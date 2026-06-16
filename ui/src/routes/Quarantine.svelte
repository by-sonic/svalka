<script lang="ts">
  import { api, type QuarantineEntry } from "../ipc";
  import { humanBytes } from "../lib/format";
  import Icon from "../lib/Icon.svelte";
  import { fly, slide, fade } from "svelte/transition";

  let entries: QuarantineEntry[] = [];
  let loading = false;
  let busyId: string | null = null;
  let confirmingId: string | null = null;
  let confirmingClear = false;
  let status = "";
  let statusKind: "ok" | "warn" | "" = "";

  async function load() {
    loading = true;
    try {
      entries = await api.listQuarantine();
      entries.sort((a, b) => b.created - a.created);
    } catch (e) {
      status = `Не удалось загрузить карантин: ${e}`;
      statusKind = "warn";
    } finally {
      loading = false;
    }
  }

  const total = (e: QuarantineEntry) => e.files.reduce((s, f) => s + f.size, 0);
  $: storedAll = entries.reduce((s, e) => s + total(e), 0);

  function when(unix: number): string {
    try {
      return new Date(unix * 1000).toLocaleString("ru-RU", {
        day: "2-digit",
        month: "short",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch {
      return "";
    }
  }

  function flash(msg: string, kind: "ok" | "warn") {
    status = msg;
    statusKind = kind;
  }

  async function restore(id: string) {
    busyId = id;
    confirmingId = null;
    try {
      await api.restoreEntry(id);
      flash("Восстановлено на место", "ok");
    } catch (e) {
      flash(`Ошибка восстановления: ${e}`, "warn");
    }
    busyId = null;
    await load();
  }

  async function deleteForever(id: string) {
    busyId = id;
    confirmingId = null;
    try {
      await api.deleteEntry(id);
      flash("Удалено навсегда", "ok");
    } catch (e) {
      flash(`Ошибка удаления: ${e}`, "warn");
    }
    busyId = null;
    await load();
  }

  async function clearAll() {
    confirmingClear = false;
    loading = true;
    try {
      const n = await api.clearQuarantine();
      flash(`Карантин очищен · удалено записей: ${n}`, "ok");
    } catch (e) {
      flash(`Ошибка очистки: ${e}`, "warn");
    }
    await load();
  }

  load();
</script>

<header class="bar">
  <span class="hicon"><Icon name="quarantine" size={20} /></span>
  <div>
    <h2>Карантин</h2>
    <div class="sub faint mono">
      {#if loading}загружаю…{:else}{entries.length} записей · {humanBytes(storedAll)} · авто-очистка 30 дней{/if}
    </div>
  </div>
  <span class="grow"></span>

  {#if entries.length && !confirmingClear}
    <button class="btn btn--danger btn--sm" on:click={() => (confirmingClear = true)} disabled={loading}>
      <Icon name="trash" size={15} /> Очистить всё
    </button>
  {:else if confirmingClear}
    <span class="confirm" transition:fade={{ duration: 120 }}>
      <span class="faint">Удалить всё навсегда?</span>
      <button class="btn btn--danger btn--sm" on:click={clearAll}>Да, удалить</button>
      <button class="btn btn--ghost btn--sm" on:click={() => (confirmingClear = false)}>Отмена</button>
    </span>
  {/if}

  <button class="btn btn--ghost btn--sm" on:click={load} disabled={loading}>
    {#if loading}<span class="spinner"></span>{:else}<Icon name="refresh" size={16} />{/if}
    Обновить
  </button>
</header>

{#if status}
  <p class="status" class:warn={statusKind === "warn"} class:ok={statusKind === "ok"} transition:fly={{ y: -4, duration: 160 }}>
    {#if statusKind === "ok"}<Icon name="check" size={15} />{:else}<Icon name="alert" size={15} />{/if}
    <span>{status}</span>
  </p>
{/if}

<div class="list">
  {#if loading && !entries.length}
    {#each Array(3) as _, i}
      <div class="card">
        <div class="skel" style="width:38px;height:38px;border-radius:10px"></div>
        <div style="flex:1">
          <div class="skel" style="height:13px;width:{40 + i * 12}%"></div>
          <div class="skel" style="height:9px;width:120px;margin-top:7px"></div>
        </div>
        <div class="skel" style="height:30px;width:170px;border-radius:8px"></div>
      </div>
    {/each}
  {:else if !entries.length}
    <div class="empty">
      <div class="ring"><Icon name="quarantine" size={28} /></div>
      <div>Карантин пуст</div>
      <div class="faint" style="font-size:12.5px">Всё удалённое попадает сюда и может быть возвращено</div>
    </div>
  {:else}
    {#each entries as e, i (e.id)}
      <div class="card" in:fly={{ y: 8, duration: 220, delay: Math.min(i * 30, 200) }} out:slide={{ duration: 160 }}>
        <span class="cicon"><Icon name="folder" size={19} /></span>
        <div class="meta">
          <div class="title">{e.source}</div>
          <div class="faint mono small">{e.files.length} объектов · {when(e.created)}</div>
        </div>
        <span class="grow"></span>
        <span class="mono amount">{humanBytes(total(e))}</span>

        {#if confirmingId === e.id}
          <span class="confirm" transition:fade={{ duration: 120 }}>
            <span class="faint">Навсегда?</span>
            <button class="btn btn--danger btn--sm" on:click={() => deleteForever(e.id)}>Да</button>
            <button class="btn btn--ghost btn--sm" on:click={() => (confirmingId = null)}>Нет</button>
          </span>
        {:else}
          <button class="btn btn--sm" on:click={() => restore(e.id)} disabled={busyId === e.id}>
            {#if busyId === e.id}<span class="spinner"></span>{:else}<Icon name="restore" size={15} />{/if}
            Вернуть
          </button>
          <button class="btn btn--danger btn--sm icon-btn" title="Удалить навсегда" on:click={() => (confirmingId = e.id)} disabled={busyId === e.id}>
            <Icon name="trash" size={15} />
          </button>
        {/if}
      </div>
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
    color: var(--cool);
    flex: 0 0 auto;
  }
  .sub {
    font-size: 12px;
    margin-top: 2px;
  }
  .confirm {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
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
  .list {
    margin-top: 16px;
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .card {
    display: flex;
    align-items: center;
    gap: 13px;
    padding: 13px 15px;
    border-radius: var(--r-md);
    background: var(--panel);
    border: 1px solid var(--line);
  }
  .cicon {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: 10px;
    background: var(--cool-soft);
    color: var(--cool);
    flex: 0 0 auto;
  }
  .meta {
    min-width: 0;
  }
  .title {
    font-size: 14px;
    font-family: var(--font-display);
  }
  .small {
    font-size: 11.5px;
    margin-top: 2px;
  }
  .amount {
    font-size: 14px;
    color: var(--text);
  }
  .icon-btn {
    padding: 7px 9px;
  }
</style>
