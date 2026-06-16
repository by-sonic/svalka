<script lang="ts">
  import { api, type QuarantineEntry } from "../ipc";
  import { humanBytes } from "../lib/format";

  let entries: QuarantineEntry[] = [];
  let status = "";

  async function load() {
    entries = await api.listQuarantine();
  }

  function total(e: QuarantineEntry): number {
    return e.files.reduce((s, f) => s + f.size, 0);
  }

  async function restore(id: string) {
    status = "Восстанавливаю...";
    try {
      await api.restoreEntry(id);
      status = "Восстановлено на место";
    } catch (e) {
      status = `Ошибка: ${e}`;
    }
    await load();
  }

  load();
</script>

<header class="bar">
  <strong>Карантин</strong>
  <span class="muted">{entries.length} записей · авто-очистка через 30 дней</span>
  <span class="grow"></span>
  <button on:click={load}>Обновить</button>
</header>

{#if status}<p class="muted">{status}</p>{/if}

{#if !entries.length}
  <p class="muted">Пусто. Всё удалённое появляется здесь и может быть возвращено.</p>
{/if}

{#each entries as e}
  <div class="row">
    <span class="meta"
      >{e.source} · {e.files.length} объектов<br /><span class="muted">{humanBytes(total(e))}</span></span
    >
    <span class="grow"></span>
    <button on:click={() => restore(e.id)}>Восстановить</button>
  </div>
{/each}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 12px;
  }
  .grow {
    flex: 1;
  }
  .muted {
    color: #888;
    font-size: 13px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px solid #80808022;
  }
  .meta {
    font-size: 14px;
  }
  button {
    padding: 7px 14px;
    border-radius: 8px;
    border: 1px solid #3b82f6;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
</style>
