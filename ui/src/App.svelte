<script lang="ts">
  import { fade, fly, scale } from "svelte/transition";
  import Icon from "./lib/Icon.svelte";
  import { api, BOOSTY_URL } from "./ipc";
  import Clean from "./routes/Clean.svelte";
  import Programs from "./routes/Programs.svelte";
  import Usage from "./routes/Usage.svelte";
  import Quarantine from "./routes/Quarantine.svelte";
  import Settings from "./routes/Settings.svelte";

  type Tab = "clean" | "programs" | "usage" | "quarantine" | "settings";
  let tab: Tab = "clean";
  let showDonate = false;

  const nav: { id: Tab; label: string; icon: string }[] = [
    { id: "clean", label: "Очистка", icon: "clean" },
    { id: "programs", label: "Программы", icon: "programs" },
    { id: "usage", label: "Анализ диска", icon: "analyze" },
    { id: "quarantine", label: "Карантин", icon: "quarantine" },
    { id: "settings", label: "Настройки", icon: "settings" },
  ];

  async function donate() {
    try {
      await api.openUrl(BOOSTY_URL);
      showDonate = false;
    } catch {
      /* keep modal open; link is shown for manual copy */
    }
  }
</script>

<div class="shell">
  <aside>
    <div class="brand">
      <span class="logo"><Icon name="logo" size={22} /></span>
      <div class="brand-text">
        <div class="wordmark">SVALKA</div>
        <div class="tag eyebrow">disk janitor</div>
      </div>
    </div>

    <nav>
      {#each nav as n}
        <button class="navitem" class:active={tab === n.id} on:click={() => (tab = n.id)}>
          <Icon name={n.icon} size={19} />
          <span>{n.label}</span>
          {#if tab === n.id}<span class="rail"></span>{/if}
        </button>
      {/each}
    </nav>

    <div class="aside-foot">
      <button class="support" on:click={() => (showDonate = true)}>
        <Icon name="heart" size={16} />
        <span>Поддержать</span>
      </button>
      <div class="ver faint">v0.1.0 · admin</div>
    </div>
  </aside>

  <main>
    {#key tab}
      <div class="screen" in:fly={{ y: 10, duration: 240, delay: 30 }} out:fade={{ duration: 90 }}>
        {#if tab === "clean"}<Clean />
        {:else if tab === "programs"}<Programs />
        {:else if tab === "usage"}<Usage />
        {:else if tab === "quarantine"}<Quarantine />
        {:else}<Settings onDonate={() => (showDonate = true)} />{/if}
      </div>
    {/key}
  </main>
</div>

<svelte:window on:keydown={(e) => { if (e.key === "Escape") showDonate = false; }} />

{#if showDonate}
  <div class="modal-bg" transition:fade={{ duration: 140 }}>
    <button class="modal-scrim" aria-label="Закрыть" on:click={() => (showDonate = false)}></button>
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-label="Поддержать проект"
      in:scale={{ start: 0.94, duration: 200 }}
      out:scale={{ start: 0.96, duration: 120 }}
    >
      <button class="modal-x" on:click={() => (showDonate = false)} aria-label="Закрыть">
        <Icon name="x" size={18} />
      </button>
      <span class="donate-emoji"><Icon name="heart" size={30} /></span>
      <h2>Спасибо, что чистишь с нами</h2>
      <p class="muted">
        Svalka бесплатна и без рекламы. Если она освободила тебе пару гигабайт —
        можешь закинуть на развитие. Любая сумма мотивирует.
      </p>
      <button class="btn btn--accent donate-cta" on:click={donate}>
        <Icon name="heart" size={16} /> Поддержать на Boosty
      </button>
      <div class="donate-link mono faint">{BOOSTY_URL}</div>
    </div>
  </div>
{/if}

<style>
  .shell {
    display: flex;
    height: 100vh;
  }

  aside {
    width: 218px;
    flex: 0 0 auto;
    padding: 18px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: linear-gradient(180deg, var(--panel) 0%, var(--bg) 100%);
    border-right: 1px solid var(--line);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 6px 8px 18px;
  }
  .logo {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: 11px;
    background: var(--acc);
    color: var(--on-acc);
    box-shadow: var(--shadow-acc);
  }
  .wordmark {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 17px;
    letter-spacing: 2.5px;
  }
  .tag {
    margin-top: 1px;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .navitem {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border: 0;
    background: transparent;
    border-radius: var(--r-sm);
    cursor: pointer;
    color: var(--text-dim);
    font-family: var(--font-display);
    font-size: 14.5px;
    letter-spacing: 0.3px;
    transition:
      color 0.15s ease,
      background 0.15s ease;
  }
  .navitem:hover {
    color: var(--text);
    background: #ffffff0a;
  }
  .navitem.active {
    color: var(--acc);
    background: var(--acc-soft);
  }
  .rail {
    position: absolute;
    left: -14px;
    top: 50%;
    transform: translateY(-50%);
    width: 3px;
    height: 22px;
    border-radius: 999px;
    background: var(--acc);
    box-shadow: 0 0 12px var(--acc-glow);
  }

  .aside-foot {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .support {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 9px;
    border-radius: var(--r-sm);
    border: 1px solid rgba(255, 111, 97, 0.35);
    background: var(--danger-soft);
    color: var(--danger);
    cursor: pointer;
    font-family: var(--font-display);
    font-size: 13.5px;
    letter-spacing: 0.3px;
    transition:
      transform 0.12s ease,
      background 0.15s ease;
  }
  .support:hover {
    transform: translateY(-1px);
    background: rgba(255, 111, 97, 0.2);
  }
  .ver {
    text-align: center;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  main {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    position: relative;
  }
  .screen {
    position: absolute;
    inset: 0;
    padding: 26px 30px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  /* donate modal */
  .modal-bg {
    position: fixed;
    inset: 0;
    background: rgba(6, 7, 9, 0.66);
    backdrop-filter: blur(4px);
    display: grid;
    place-items: center;
    z-index: 50;
  }
  .modal-scrim {
    position: absolute;
    inset: 0;
    border: 0;
    padding: 0;
    background: transparent;
    cursor: default;
  }
  .modal {
    position: relative;
    z-index: 1;
    width: min(420px, 90vw);
    padding: 30px 28px 24px;
    border-radius: var(--r-lg);
    background: var(--panel-2);
    border: 1px solid var(--line-strong);
    box-shadow: var(--shadow);
    text-align: center;
  }
  .modal-x {
    position: absolute;
    top: 14px;
    right: 14px;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    border: 0;
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
  }
  .modal-x:hover {
    background: #ffffff0d;
    color: var(--text);
  }
  .donate-emoji {
    display: inline-grid;
    place-items: center;
    width: 60px;
    height: 60px;
    border-radius: 50%;
    background: var(--danger-soft);
    color: var(--danger);
    margin-bottom: 14px;
  }
  .modal h2 {
    font-size: 19px;
    margin-bottom: 8px;
  }
  .modal p {
    font-size: 13.5px;
    line-height: 1.55;
    margin: 0 0 20px;
  }
  .donate-cta {
    width: 100%;
    justify-content: center;
  }
  .donate-link {
    margin-top: 12px;
    font-size: 11.5px;
    word-break: break-all;
  }
</style>
