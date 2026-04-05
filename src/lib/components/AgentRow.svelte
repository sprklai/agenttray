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

<div
  role="group"
  class="flex items-center gap-[9px] px-3 py-[7px] cursor-default transition-colors"
  class:bg-[#252525]={hovered}
  onmouseenter={() => hovered = true}
  onmouseleave={() => hovered = false}
>
  <StatusDot status={agent.status} />

  <div class="flex-1 min-w-0">
    <p class="text-[12px] font-medium text-[#dddbd5] truncate">{agent.name}</p>
    <p class="text-[11px] text-[#7a7870] truncate mt-[1px]">
      {agent.message || STATUS_LABEL[agent.status]}
    </p>
  </div>

  <div class="flex flex-col items-end gap-[3px] flex-shrink-0">
    <span class="text-[10px] text-[#7a7870] opacity-60 max-w-[100px] truncate text-right" title={agent.terminal?.window_title ?? agent.terminal?.label ?? ''}>
      {agent.terminal?.window_title ?? agent.terminal?.label ?? ''}
    </span>
    {#if agent.can_focus}
      <button
        class="text-[10px] font-medium px-[7px] py-[2px] rounded border transition-all
               text-[#4898cc] border-[rgba(72,152,204,0.35)] bg-[rgba(72,152,204,0.1)]
               hover:bg-[rgba(72,152,204,0.2)] hover:border-[rgba(72,152,204,0.55)]"
        class:opacity-0={!hovered}
        class:opacity-100={hovered}
        onclick={onFocus}
      >
        focus ↗
      </button>
    {/if}
  </div>
</div>
