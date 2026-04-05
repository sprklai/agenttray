<script lang="ts">
  import type { AgentStatus } from '$lib/types';
  import { STATUS_LABEL } from '$lib/types';
  import StatusDot from './StatusDot.svelte';

  let {
    agent,
    onFocus,
  }: { agent: AgentStatus; onFocus: () => void } = $props();

  let hovered = $state(false);
</script>

<li
  class="flex items-center gap-[9px] px-3.5 py-[7px] cursor-default transition-all duration-150 rounded-lg mx-1 list-none"
  style="background: {hovered ? 'rgba(255,255,255,0.08)' : 'transparent'}; {hovered ? 'box-shadow: inset 0 0.5px 0 rgba(255,255,255,0.06);' : ''}"
  onmouseenter={() => hovered = true}
  onmouseleave={() => hovered = false}
>
  <StatusDot status={agent.status} />

  <div class="flex-1 min-w-0">
    <p class="text-[12px] font-medium text-[#e8e6e1] truncate">{agent.name}</p>
    <p class="text-[11px] text-[#8a8880] truncate mt-[1px]">
      {agent.message || STATUS_LABEL[agent.status]}
    </p>
  </div>

  <div class="flex flex-col items-end gap-[3px] flex-shrink-0">
    <span class="text-[10px] text-[#8a8880] opacity-60 max-w-[100px] truncate text-right" title={agent.terminal?.window_title ?? agent.terminal?.label ?? ''}>
      {agent.terminal?.window_title ?? agent.terminal?.label ?? ''}
    </span>
    {#if agent.can_focus}
      <button
        type="button"
        aria-label="Focus terminal for {agent.name}"
        class="text-[10px] font-medium px-[7px] py-[2px] rounded-md border transition-all duration-150
               text-[#5aa8dc] border-[rgba(90,168,220,0.25)] bg-[rgba(90,168,220,0.08)]
               hover:bg-[rgba(90,168,220,0.18)] hover:border-[rgba(90,168,220,0.45)]
               disabled:opacity-30"
        style="backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px);"
        class:opacity-0={!hovered}
        class:opacity-100={hovered}
        onclick={onFocus}
      >
        focus ↗
      </button>
    {/if}
  </div>
</li>
