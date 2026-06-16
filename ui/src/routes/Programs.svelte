<script lang="ts">
  import { api, type Program } from "../ipc";
  import { humanBytes } from "../lib/format";

  let progs: Program[] = [];
  let selected = new Set<string>();
  let loading = false;
  let status = "";

  async function load() {
    loading = true;
    progs = await api.getPrograms();
    progs.sort((a, b) => b.real_size - a.real_size);
    loading = false;
  }

  function toggle(name: string) {
    selected.has(name) ? selected.delete(name) : selected.add(name);
    selected = selected;
  }

  async function uninstall() {
    const chosen = progs.filter((p) => selected.has(p.name));
    if (!chosen.length) return;
    status = `Удаляю ${chosen.length}...`;
    const results = await api.uninstallPrograms(chosen);
    const failed = results.filter((r) => !r.ok);
    status = failed.length
      ? `Готово, но не удалось: ${failed.map((f) => `${f.program} (${f.message})`).join("; ")}`
      : "Все выбранные удалены";
    selected = new Set();
    await load();
  }

  load();
</script>

<header class="bar">
  <strong>Программы</strong>
  <span class="muted">{progs.length} шт.</span>
  <span class="grow"></span>
  <button on:click={load} disabled={loading}>Обновить</button>
  <button on:click={uninstall} disabled={!selected.size}>Удалить выбранные ({selected.size})</button>
</header>

{#if status}<p class="muted">{status}</p>{/if}

{#each progs as p}
  <label class="row">
    <input type="checkbox" checked={selected.has(p.name)} on:change={() => toggle(p.name)} />
    <span class="name">{p.name}<br /><span class="muted">{p.publisher ?? ""}</span></span>
    <span class="muted">{p.real_size ? humanBytes(p.real_size) : "—"}</span>
  </label>
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
    padding: 8px 0;
    border-bottom: 1px solid #80808022;
  }
  .name {
    flex: 1;
    font-size: 14px;
  }
  button {
    padding: 7px 14px;
    border-radius: 8px;
    border: 1px solid #3b82f6;
    background: #3b82f6;
    color: #fff;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
