<script lang="ts">
  import type { Status } from '$lib/types';
  import { STATUS_COLOR } from '$lib/types';

  let { status }: { status: Status } = $props();

  const animated: ReadonlySet<string> = new Set(['needs-input', 'working', 'starting']);
  const isAnimated = $derived(animated.has(status));
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
    box-shadow: 0 0 6px 1.5px {color}60;
    animation: {isAnimated ? `blink ${animDuration} ease-in-out infinite` : 'none'};
  "
></div>
