<script lang="ts">
  // Stroke-based icon set. Inherits color via currentColor, scales crisply.
  export let name: string;
  export let size: number = 20;
  export let stroke: number = 1.7;
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === "clean"}
    <!-- sparkle / fresh -->
    <path d="M5 13l1.6 3.4L10 18l-3.4 1.6L5 23l-1.6-3.4L0 18l3.4-1.6z" transform="translate(3 -2) scale(0.8)" />
    <path d="M14 3l1.9 4.1L20 9l-4.1 1.9L14 15l-1.9-4.1L8 9l4.1-1.9z" />
  {:else if name === "programs"}
    <rect x="3" y="3" width="7" height="7" rx="1.5" />
    <rect x="14" y="3" width="7" height="7" rx="1.5" />
    <rect x="3" y="14" width="7" height="7" rx="1.5" />
    <rect x="14" y="14" width="7" height="7" rx="1.5" />
  {:else if name === "quarantine"}
    <path d="M12 3l7 3v5c0 4.4-3 8-7 10-4-2-7-5.6-7-10V6z" />
    <path d="M9 12l2 2 4-4" />
  {:else if name === "settings"}
    <line x1="4" y1="7" x2="20" y2="7" />
    <line x1="4" y1="17" x2="20" y2="17" />
    <circle cx="9" cy="7" r="2.4" fill="var(--bg)" />
    <circle cx="15" cy="17" r="2.4" fill="var(--bg)" />
  {:else if name === "refresh"}
    <path d="M20 11a8 8 0 1 0-2.3 6" />
    <path d="M20 4v6h-6" />
  {:else if name === "trash"}
    <path d="M4 7h16" />
    <path d="M9 7V4h6v3" />
    <path d="M6 7l1 13h10l1-13" />
    <line x1="10" y1="11" x2="10" y2="17" />
    <line x1="14" y1="11" x2="14" y2="17" />
  {:else if name === "restore"}
    <path d="M3 9a9 9 0 1 1-1.5 5" />
    <path d="M3 4v5h5" />
  {:else if name === "heart"}
    <path d="M12 20s-7-4.6-9.3-9.1C1.2 8.1 2.6 5 5.7 5c1.9 0 3.2 1.1 4.1 2.4C10.7 6.1 12 5 13.9 5 17 5 18.4 8.1 17.1 10.9 14.8 15.4 12 20 12 20z" />
  {:else if name === "check"}
    <path d="M4 12l5 5L20 6" />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="7" />
    <line x1="21" y1="21" x2="16.5" y2="16.5" />
  {:else if name === "drive"}
    <rect x="3" y="5" width="18" height="14" rx="2" />
    <line x1="3" y1="13" x2="21" y2="13" />
    <circle cx="17" cy="16.5" r="0.9" fill="currentColor" stroke="none" />
  {:else if name === "folder"}
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2.5h8a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
  {:else if name === "chevron"}
    <path d="M9 6l6 6-6 6" />
  {:else if name === "x"}
    <line x1="6" y1="6" x2="18" y2="18" />
    <line x1="18" y1="6" x2="6" y2="18" />
  {:else if name === "alert"}
    <path d="M12 4l9 16H3z" />
    <line x1="12" y1="10" x2="12" y2="14" />
    <circle cx="12" cy="17" r="0.6" fill="currentColor" stroke="none" />
  {:else if name === "logo"}
    <!-- stylised trash funnel / sweep -->
    <path d="M4 5h16l-2.5 5H6.5z" />
    <path d="M7 13h10M8.5 17h7M10 21h4" />
  {:else if name === "analyze"}
    <rect x="3" y="3" width="8" height="11" rx="1.5" />
    <rect x="13" y="3" width="8" height="6" rx="1.5" />
    <rect x="13" y="11" width="8" height="10" rx="1.5" />
    <rect x="3" y="16" width="8" height="5" rx="1.5" />
  {/if}
</svg>
