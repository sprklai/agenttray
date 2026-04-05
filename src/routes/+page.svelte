<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import type { AgentStatus } from '$lib/types';
  import AgentRow from '$lib/components/AgentRow.svelte';
  import AggregatePill from '$lib/components/AggregatePill.svelte';
  import { aggregate } from '$lib/utils';

  let agents = $state<AgentStatus[]>([]);
  let pinned = $state(false);
  let aggregateState = $derived(aggregate(agents));

  onMount(async () => {
    const unlisten = await listen<AgentStatus[]>('agents-updated', (event) => {
      agents = event.payload;
    });

    const unlistenPin = await listen<boolean>('pinned-changed', (event) => {
      pinned = event.payload;
    });

    const win = getCurrentWindow();
    let showTime = Date.now();

    // Track when the window becomes visible so we can ignore
    // immediate blur events (e.g. from global shortcut toggle)
    const unlistenShow = await listen('tauri://focus', () => {
      showTime = Date.now();
    });

    const unlistenBlur = await win.onFocusChanged(({ payload: focused }) => {
      if (!focused && !pinned && Date.now() - showTime > 300) win.hide();
    });

    return () => { unlisten(); unlistenPin(); unlistenShow(); unlistenBlur(); };
  });

  async function focusAgent(agent: AgentStatus) {
    if (!agent.can_focus || !agent.terminal) return;
    await invoke('focus_terminal', {
      req: {
        kind:     agent.terminal.kind,
        focus_id: agent.terminal.focus_id,
        outer_id: agent.terminal.outer_id,
      }
    });
    getCurrentWindow().hide();
  }
</script>

<div class="w-[292px] rounded-[10px] overflow-hidden bg-[#1c1c1c] border border-white/10 shadow-2xl m-[4px]">
  <!-- Header -->
  <div class="flex items-center justify-between px-3 py-2 border-b border-white/[0.07]">
    <span class="text-[10px] font-semibold tracking-widest uppercase text-[#7a7870]">Agents</span>
    <AggregatePill state={aggregateState} />
  </div>

  <!-- Agent list -->
  <div class="py-1">
    {#if agents.length === 0}
      <p class="text-[11px] text-[#7a7870] text-center py-5 leading-relaxed">
        No agents detected.<br/>See ~/.agent-monitor/
      </p>
    {:else}
      {#each agents as agent (agent.name)}
        <AgentRow {agent} onFocus={() => focusAgent(agent)} />
      {/each}
    {/if}
  </div>
</div>
