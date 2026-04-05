<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import type { AgentStatus } from '$lib/types';
  import AgentRow from '$lib/components/AgentRow.svelte';
  import AggregatePill from '$lib/components/AggregatePill.svelte';
  import { aggregate } from '$lib/utils';
  import Pin from '@lucide/svelte/icons/pin';
  import PinOff from '@lucide/svelte/icons/pin-off';

  let agents = $state<AgentStatus[]>([]);
  let pinned = $state(false);
  let statusDir = $state('~/.agent-monitor');
  let aggregateState = $derived(aggregate(agents));

  onMount(() => {
    const cleanups: Array<() => void> = [];

    (async () => {
      // Fetch cached state immediately (no 200ms delay waiting for backend push)
      agents = await invoke<AgentStatus[]>('get_agents');
      statusDir = await invoke<string>('get_status_dir') || statusDir;

      cleanups.push(await listen<AgentStatus[]>('agents-updated', (event) => {
        agents = event.payload;
      }));

      cleanups.push(await listen<boolean>('pinned-changed', (event) => {
        pinned = event.payload;
      }));

      const win = getCurrentWindow();
      let showTime = Date.now();

      // Track when the window becomes visible so we can ignore
      // immediate blur events (e.g. from global shortcut toggle)
      cleanups.push(await listen('tauri://focus', () => {
        showTime = Date.now();
      }));

      // Ignore blur events within 300ms of focus — the global shortcut
      // toggle causes a brief focus/blur cycle that would immediately hide the popup
      const BLUR_DEBOUNCE_MS = 300;
      cleanups.push(await win.onFocusChanged(({ payload: focused }) => {
        if (!focused && !pinned && Date.now() - showTime > BLUR_DEBOUNCE_MS) win.hide();
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
      getCurrentWindow().hide();
    } catch (e) {
      focusError = String(e);
      setTimeout(() => { focusError = ''; }, 4000);
    } finally {
      focusing = false;
    }
  }
</script>

<div class="glass-panel glass-noise relative w-[292px] rounded-[14px] overflow-hidden m-[4px]">
  <!-- Header (drag region) -->
  <div data-tauri-drag-region class="flex items-center justify-between px-3.5 py-2.5 border-b border-white/[0.06]"
       style="background: rgba(255,255,255,0.03);">
    <span data-tauri-drag-region class="text-[10px] font-semibold tracking-widest uppercase text-[#8a8880]">Agents</span>
    <div class="flex items-center gap-1.5">
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
      <AggregatePill state={aggregateState} />
    </div>
  </div>

  <!-- Focus error toast -->
  {#if focusError}
    <div class="px-3 py-1.5 bg-[#3a2020] border-b border-red-500/20 text-[10px] text-red-400 truncate">
      {focusError}
    </div>
  {/if}

  <!-- Agent list -->
  {#if agents.length === 0}
    <p class="text-[11px] text-[#8a8880] text-center py-5 leading-relaxed">
      No agents detected.<br/>See {statusDir}
    </p>
  {:else}
    <ul class="py-1 m-0 p-0 glass-scroll" style="max-height: 360px; overflow-y: auto;">
      {#each agents as agent (agent.name)}
        <AgentRow {agent} onFocus={() => focusAgent(agent)} />
      {/each}
    </ul>
  {/if}
</div>
