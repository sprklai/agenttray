<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { LogicalSize } from '@tauri-apps/api/dpi';
  import type { AgentStatus } from '$lib/types';
  import AgentRow from '$lib/components/AgentRow.svelte';
  import AggregatePill from '$lib/components/AggregatePill.svelte';
  import { aggregate } from '$lib/utils';
  import Pin from '@lucide/svelte/icons/pin';
  import PinOff from '@lucide/svelte/icons/pin-off';

  // Window width and max list rows before scroll kicks in
  const WIN_W = 400;
  const MAX_VISIBLE = 5;
  const ROW_H = 48;
  const MAX_LIST_H = MAX_VISIBLE * ROW_H; // 240px cap, then scroll

  let agents = $state<AgentStatus[]>([]);
  let pinned = $state(false);
  let statusDir = $state('~/.agent-monitor');
  let ipcError = $state('');
  let aggregateState = $derived(aggregate(agents));

  // Panel ref for ResizeObserver-based window sizing
  let panelEl = $state<HTMLDivElement | null>(null);

  // Auto-resize window to match actual rendered panel height
  $effect(() => {
    if (!panelEl) return;
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      // borderBoxSize includes border but not margin; add 8px for m-[4px] × 2
      const borderBoxH = entry.borderBoxSize[0]?.blockSize ?? entry.contentRect.height + 2;
      const totalH = Math.round(borderBoxH) + 8;
      getCurrentWindow().setSize(new LogicalSize(WIN_W, totalH));
    });
    observer.observe(panelEl);
    return () => observer.disconnect();
  });

  onMount(() => {
    const cleanups: Array<() => void> = [];

    (async () => {
      // Register listeners FIRST to avoid missing events between
      // get_agents() and listener registration (race condition fix)
      cleanups.push(await listen<AgentStatus[]>('agents-updated', (event) => {
        agents = event.payload;
        ipcError = ''; // clear any previous error on successful event
      }));

      cleanups.push(await listen<boolean>('pinned-changed', (event) => {
        pinned = event.payload;
      }));

      // Now fetch cached state (listener is already active for any updates)
      try {
        const result = await invoke<AgentStatus[]>('get_agents');
        console.log('[AgentTray] get_agents returned', result.length, 'agents');
        agents = result;
      } catch (e) {
        ipcError = `IPC error: ${e}`;
        console.error('[AgentTray] get_agents failed:', e);
      }

      // Retry once if initial fetch returned empty (backend may not have
      // completed first scan yet on cold start)
      if (agents.length === 0) {
        await new Promise(r => setTimeout(r, 600));
        try {
          const retry = await invoke<AgentStatus[]>('get_agents');
          console.log('[AgentTray] retry get_agents returned', retry.length, 'agents');
          if (retry.length > 0) agents = retry;
        } catch {}
      }

      try {
        statusDir = await invoke<string>('get_status_dir') || statusDir;
      } catch {}

      const win = getCurrentWindow();
      let showTime = Date.now();

      // Re-fetch agents when popup is shown (also works around HMR state loss)
      cleanups.push(await listen('tauri://focus', async () => {
        showTime = Date.now();
        try {
          agents = await invoke<AgentStatus[]>('get_agents');
        } catch {}
      }));

      // Ignore blur events within 300ms of focus — the global shortcut
      // toggle causes a brief focus/blur cycle that would immediately hide the popup
      const BLUR_DEBOUNCE_MS = 300;
      cleanups.push(await win.onFocusChanged(({ payload: focused }) => {
        if (!focused && !pinned && Date.now() - showTime > BLUR_DEBOUNCE_MS) invoke('close_popup');
      }));
    })();

    return () => cleanups.forEach(fn => fn());
  });

  let focusing = $state(false);
  let focusError = $state('');
  async function focusAgent(agent: AgentStatus) {
    if (focusing || !agent.can_focus || !agent.terminal) return;
    focusing = true;
    focusError = '';
    try {
      await invoke('focus_terminal', {
        req: {
          kind:     agent.terminal.kind,
          focus_id: agent.terminal.focus_id,
          outer_id: agent.terminal.outer_id,
        }
      });
      if (!pinned) invoke('close_popup');
    } catch (e) {
      focusError = String(e);
      setTimeout(() => { focusError = ''; }, 4000);
    } finally {
      focusing = false;
    }
  }
</script>

<div bind:this={panelEl} class="glass-panel glass-noise relative w-[392px] rounded-[14px] overflow-hidden m-[4px]">
  <!-- Header (drag region) -->
  <div data-tauri-drag-region class="flex items-center justify-between px-3.5 py-2.5 border-b border-white/[0.06]"
       style="background: rgba(255,255,255,0.03);">
    <span data-tauri-drag-region class="text-[10px] font-semibold tracking-widest uppercase text-[#8a8880]">AgentTray</span>
    <div class="flex items-center gap-1.5">
      <AggregatePill state={aggregateState} />
      <button
        onclick={() => invoke('toggle_pin')}
        class="p-0.5 rounded hover:bg-white/10 transition-colors {pinned ? 'text-white/80' : 'text-[#7a7870]'}"
        title={pinned ? 'Unpin window' : 'Pin window'}
      >
        {#if pinned}
          <Pin size={12} />
        {:else}
          <PinOff size={12} />
        {/if}
      </button>
    </div>
  </div>

  <!-- Error toasts -->
  {#if focusError}
    <div class="px-3 py-1.5 bg-[#3a2020] border-b border-red-500/20 text-[10px] text-red-400 truncate">
      {focusError}
    </div>
  {/if}
  {#if ipcError}
    <div class="px-3 py-1.5 bg-[#3a2020] border-b border-red-500/20 text-[10px] text-red-400 truncate">
      {ipcError}
    </div>
  {/if}

  <!-- Agent list -->
  {#if agents.length === 0}
    <p class="text-[11px] text-[#8a8880] text-center py-5 leading-relaxed">
      No agents detected.<br/>See {statusDir}
    </p>
  {:else}
    <ul class="py-1 m-0 p-0 glass-scroll" style="max-height: {MAX_LIST_H}px; overflow-y: auto; overflow-x: hidden;">
      {#each agents as agent (agent.id)}
        <AgentRow {agent} onFocus={() => focusAgent(agent)} />
      {/each}
    </ul>
  {/if}
</div>
