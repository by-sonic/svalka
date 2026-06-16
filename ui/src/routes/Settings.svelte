<script lang="ts">
  import Icon from "../lib/Icon.svelte";
  export let onDonate: () => void = () => {};

  const facts: { icon: string; title: string; body: string }[] = [
    {
      icon: "quarantine",
      title: "Ничего не удаляется навсегда",
      body: "Очистка переносит мусор в карантин на том же диске. Вернуть можно в любой момент на вкладке «Карантин».",
    },
    {
      icon: "restore",
      title: "Авто-очистка карантина",
      body: "Записи старше 30 дней удаляются автоматически при запуске — карантин не разрастается.",
    },
    {
      icon: "check",
      title: "Права администратора",
      body: "Нужны, чтобы чистить системные папки и создавать карантин в корне диска. Запрашиваются при запуске.",
    },
  ];
</script>

<header class="bar">
  <span class="hicon"><Icon name="settings" size={20} /></span>
  <div>
    <h2>Настройки</h2>
    <div class="sub faint mono">Svalka v0.1.0</div>
  </div>
</header>

<div class="grid">
  {#each facts as f}
    <div class="card">
      <span class="cicon"><Icon name={f.icon} size={18} /></span>
      <div>
        <div class="ctitle">{f.title}</div>
        <p class="muted">{f.body}</p>
      </div>
    </div>
  {/each}
</div>

<div class="support-card">
  <span class="heart"><Icon name="heart" size={22} /></span>
  <div class="sc-text">
    <div class="ctitle">Нравится Svalka?</div>
    <p class="muted">Бесплатно и без рекламы. Поддержи разработку, если чистилка пригодилась.</p>
  </div>
  <button class="btn btn--accent" on:click={onDonate}>
    <Icon name="heart" size={16} /> Поддержать
  </button>
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
  .grid {
    margin-top: 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .card {
    display: flex;
    gap: 14px;
    padding: 16px;
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
    background: var(--acc-soft);
    color: var(--acc);
    flex: 0 0 auto;
  }
  .ctitle {
    font-family: var(--font-display);
    font-size: 14.5px;
    margin-bottom: 4px;
  }
  .muted {
    margin: 0;
    font-size: 13px;
    line-height: 1.5;
  }
  .support-card {
    margin-top: 16px;
    display: flex;
    align-items: center;
    gap: 15px;
    padding: 18px;
    border-radius: var(--r-md);
    background: linear-gradient(120deg, var(--danger-soft), transparent 70%), var(--panel);
    border: 1px solid rgba(255, 111, 97, 0.28);
  }
  .heart {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: var(--danger-soft);
    color: var(--danger);
    flex: 0 0 auto;
  }
  .sc-text {
    flex: 1;
    min-width: 0;
  }
</style>
