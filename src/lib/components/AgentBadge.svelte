<script lang="ts">
  import type { AgentCli, Status } from '$lib/types';
  import { STATUS_COLOR } from '$lib/types';
  import CliIcon from './CliIcon.svelte';

  let {
    cli,
    status,
  }: { cli: AgentCli; status: Status } = $props();

  let statusColor = $derived(STATUS_COLOR[status] || '#555');

  // Animated statuses get the blink animation
  let animClass = $derived(
    status === 'needs-input' ? 'animate-badge-blink-fast'
    : status === 'working' ? 'animate-badge-blink'
    : status === 'starting' ? 'animate-badge-blink-slow'
    : ''
  );
</script>

<div
  class="relative flex items-center justify-center w-[20px] h-[20px] rounded-full flex-shrink-0 {animClass}"
  style="
    border: 2px solid {statusColor};
    box-shadow: 0 0 6px 1.5px {statusColor}50;
    background: {statusColor}10;
  "
>
  <CliIcon {cli} size={12} />
</div>

<style>
  .animate-badge-blink-fast {
    animation: blink 0.9s ease-in-out infinite;
  }
  .animate-badge-blink {
    animation: blink 1.3s ease-in-out infinite;
  }
  .animate-badge-blink-slow {
    animation: blink 1.7s ease-in-out infinite;
  }
</style>
