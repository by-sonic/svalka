<script lang="ts">
  import Clean from "./routes/Clean.svelte";
  import Programs from "./routes/Programs.svelte";
  import Quarantine from "./routes/Quarantine.svelte";
  import Settings from "./routes/Settings.svelte";

  type Tab = "clean" | "programs" | "quarantine" | "settings";
  let tab: Tab = "clean";
  const nav: { id: Tab; label: string }[] = [
    { id: "clean", label: "Очистка" },
    { id: "programs", label: "Программы" },
    { id: "quarantine", label: "Карантин" },
    { id: "settings", label: "Настройки" },
  ];
</script>

<div class="shell">
  <aside>
    <div class="brand">Svalka</div>
    {#each nav as n}
      <button class:active={tab === n.id} on:click={() => (tab = n.id)}>{n.label}</button>
    {/each}
  </aside>
  <main>
    {#if tab === "clean"}<Clean />
    {:else if tab === "programs"}<Programs />
    {:else if tab === "quarantine"}<Quarantine />
    {:else}<Settings />{/if}
  </main>
</div>

<style>
  .shell {
    display: flex;
    height: 100vh;
    font-family: system-ui, sans-serif;
  }
  aside {
    width: 180px;
    border-right: 1px solid #80808033;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .brand {
    font-weight: 600;
    padding: 8px 10px;
    margin-bottom: 8px;
  }
  aside button {
    text-align: left;
    padding: 8px 10px;
    border: 0;
    background: transparent;
    border-radius: 8px;
    cursor: pointer;
    color: inherit;
    font-size: 14px;
  }
  aside button.active {
    background: #3b82f6;
    color: #fff;
  }
  main {
    flex: 1;
    padding: 20px;
    overflow: auto;
  }
</style>
