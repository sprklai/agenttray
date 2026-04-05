<script lang="ts">
  import type { Status } from '$lib/types';
  import { STATUS_COLOR } from '$lib/types';

  let { status }: { status: Status } = $props();

  const animated = ['needs-input', 'working', 'starting'] as const;
  const isAnimated = $derived((animated as readonly string[]).includes(status));
  const color = $derived(STATUS_COLOR[status]);
  const animDuration = $derived(
    status === 'needs-input' ? '0.9s' :
    status === 'working'     ? '1.3s' : '1.7s'
  );
</script>

<div
  class="w-[7px] h-[7px] rounded-full flex-shrink-0"
  style="
    background-color: {color};
    animation: {isAnimated ? `blink ${animDuration} ease-in-out infinite` : 'none'};
  "
></div>
