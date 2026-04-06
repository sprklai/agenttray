<script lang="ts">
  import type { AgentStatus } from '$lib/types';
  import { STATUS_LABEL, CLI_LABEL, CLI_COLOR, formatTerminalChip } from '$lib/types';
  import StatusDot from './StatusDot.svelte';
  import { SquareTerminal } from '@lucide/svelte';

  let {
    agent,
    onFocus,
  }: { agent: AgentStatus; onFocus: () => void } = $props();

  let hovered = $state(false);

  let cliLabel = $derived(agent.cli ? CLI_LABEL[agent.cli] || '' : '');
  let cliColor = $derived(agent.cli ? CLI_COLOR[agent.cli] || '#888' : '#888');
  let termChip = $derived(formatTerminalChip(agent.terminal));
</script>

<li
  class="flex items-center gap-[9px] px-3.5 py-[7px] cursor-default transition-all duration-150 rounded-lg mx-1 list-none"
  style="background: {hovered ? 'rgba(255,255,255,0.08)' : 'transparent'}; {hovered ? 'box-shadow: inset 0 0.5px 0 rgba(255,255,255,0.06);' : ''}"
  onmouseenter={() => hovered = true}
  onmouseleave={() => hovered = false}
>
  <StatusDot status={agent.status} />

  <div class="flex-1 min-w-0">
    <div class="flex items-center gap-1.5">
      <p class="text-[12px] font-medium text-[#e8e6e1] truncate">{agent.name}</p>
      {#if cliLabel}
        <span
          class="text-[9px] font-semibold px-[5px] py-[1px] rounded-[3px] flex-shrink-0 uppercase tracking-wide"
          style="color: {cliColor}; background: {cliColor}18; border: 0.5px solid {cliColor}30;"
        >{cliLabel}</span>
      {/if}
    </div>
    <p class="text-[11px] text-[#8a8880] truncate mt-[1px]">
      {#if termChip}
        <span class="inline-flex items-center gap-[3px] text-[#9a9890]">
          <SquareTerminal size={11} strokeWidth={1.8} class="inline-block flex-shrink-0" />
          <span>{termChip}</span>
        </span>
        {#if agent.message}
          <span class="text-[#6a6860]"> — </span>{agent.message}
        {/if}
      {:else}
        {agent.message || STATUS_LABEL[agent.status]}
      {/if}
    </p>
  </div>

  <div class="flex flex-col items-end gap-[3px] flex-shrink-0">
    {#if agent.cpu != null && agent.cpu > 0}
      <span class="text-[10px] font-mono text-[#8a8880] opacity-80 tabular-nums">
        {agent.cpu.toFixed(0)}% CPU
      </span>
    {/if}
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
