<script lang="ts">
  import type { AgentStatus } from '$lib/types';
  import { STATUS_LABEL } from '$lib/types';
  import AgentBadge from './AgentBadge.svelte';

  let {
    agent,
    onFocus,
  }: { agent: AgentStatus; onFocus: () => void } = $props();

  let hovered = $state(false);
</script>

<li
  class="flex items-center gap-2 px-3.5 py-[7px] cursor-default transition-all duration-150 rounded-lg mx-1 list-none"
  style="background: {hovered ? 'rgba(255,255,255,0.08)' : 'transparent'}; {hovered ? 'box-shadow: inset 0 0.5px 0 rgba(255,255,255,0.06);' : ''}"
  onmouseenter={() => hovered = true}
  onmouseleave={() => hovered = false}
>
  <AgentBadge cli={agent.cli ?? 'unknown'} status={agent.status} />

  <div class="flex-1 min-w-0">
    <div class="flex items-center justify-between gap-2">
      <p class="text-[12px] font-medium text-[#e8e6e1] truncate">{agent.name}</p>
      {#if agent.cpu != null && agent.cpu > 0}
        <span class="text-[10px] font-mono text-[#8a8880] opacity-80 tabular-nums flex-shrink-0">
          {agent.cpu.toFixed(0)}%
        </span>
      {:else}
        <span class="text-[10px] text-[#8a8880] opacity-60 max-w-[80px] truncate text-right flex-shrink-0" title={agent.terminal?.window_title ?? agent.terminal?.label ?? ''}>
          {agent.terminal?.window_title ?? agent.terminal?.label ?? ''}
        </span>
      {/if}
    </div>
    <div class="flex items-center justify-between gap-2 mt-[1px]">
      <p class="text-[11px] text-[#8a8880] truncate">
        {agent.message || STATUS_LABEL[agent.status]}
      </p>
      {#if agent.can_focus}
        <button
          type="button"
          aria-label="Focus terminal for {agent.name}"
          class="text-[10px] font-medium px-[7px] py-[1px] rounded-md border transition-all duration-150 flex-shrink-0
                 text-[#5aa8dc] border-[rgba(90,168,220,0.25)] bg-[rgba(90,168,220,0.08)]
                 hover:bg-[rgba(90,168,220,0.18)] hover:border-[rgba(90,168,220,0.45)]
                 disabled:opacity-30"
          style="backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px);"
          class:opacity-0={!hovered}
          class:opacity-100={hovered}
          onclick={onFocus}
        >
          focus &#8599;
        </button>
      {/if}
    </div>
  </div>
</li>
