<script lang="ts">
  let purgeDays = 30;
</script>

<h2>Настройки</h2>
<label class="row">
  Авто-очистка карантина через (дней)
  <input type="number" min="1" max="365" bind:value={purgeDays} />
</label>
<p class="muted">
  Версия 0.1.0 · Svalka работает с правами администратора, чтобы чистить системные папки.
</p>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 16px 0;
  }
  .muted {
    color: #888;
    font-size: 13px;
  }
  input {
    width: 80px;
    padding: 6px;
  }
</style>
