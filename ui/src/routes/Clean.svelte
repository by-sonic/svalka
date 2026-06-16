<script lang="ts">
  import { api, type Category, type CategoryId } from "../ipc";
  import { humanBytes } from "../lib/format";

  const DEFAULT: CategoryId[] = ["SystemTemp", "BrowserCache", "DevCache"];
  let cats: Category[] = [];
  let selected = new Set<string>();
  let scanning = false;
  let status = "";

  async function scan() {
    scanning = true;
    status = "Сканирую...";
    cats = await api.scan(DEFAULT);
    selected = new Set(cats.flatMap((c) => c.items.map((i) => i.path)));
    scanning = false;
    status = "";
  }

  function toggle(path: string) {
    selected.has(path) ? selected.delete(path) : selected.add(path);
    selected = selected;
  }

  $: total = cats
    .flatMap((c) => c.items)
    .filter((i) => selected.has(i.path))
    .reduce((s, i) => s + i.size, 0);

  function volumeRoot(p: string): string {
    return p.slice(0, 2) + "\\";
  }

  async function clean() {
    const paths = [...selected];
    if (!paths.length) return;
    status = "Отправляю в карантин...";
    const byVol = new Map<string, string[]>();
    for (const p of paths) {
      const v = volumeRoot(p);
      byVol.set(v, [...(byVol.get(v) ?? []), p]);
    }
    let freed = 0;
    for (const [v, ps] of byVol) {
      const r = await api.quarantineItems("Очистка", v, ps);
      if (r.ok) freed += r.freed;
    }
    status = `Освобождено ${humanBytes(freed)} — всё в карантине, можно вернуть`;
    await scan();
  }
</script>

<header class="bar">
  <strong>Очистка</strong>
  <span class="muted">{cats.length ? `Выбрано ${humanBytes(total)}` : "Нажмите «Сканировать»"}</span>
  <span class="grow"></span>
  <button on:click={scan} disabled={scanning}>Сканировать</button>
  <button on:click={clean} disabled={!selected.size}>Отправить в карантин</button>
</header>

{#if status}<p class="muted">{status}</p>{/if}

{#each cats as c}
  <section>
    <h3>{c.label} <span class="muted">{humanBytes(c.items.reduce((s, i) => s + i.size, 0))}</span></h3>
    {#each c.items as i}
      <label class="row">
        <input type="checkbox" checked={selected.has(i.path)} on:change={() => toggle(i.path)} />
        <span class="path">{i.path}</span>
        <span class="muted">{humanBytes(i.size)}</span>
      </label>
    {/each}
  </section>
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
    gap: 10px;
    padding: 6px 0;
    border-bottom: 1px solid #80808022;
  }
  .path {
    flex: 1;
    font-size: 13px;
    word-break: break-all;
  }
  h3 {
    font-size: 15px;
    margin: 16px 0 6px;
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
