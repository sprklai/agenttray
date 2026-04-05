<script lang="ts">
  import type { Status } from '$lib/types';
  import { STATUS_LABEL, STATUS_COLOR } from '$lib/types';

  let { state }: { state: Status } = $props();
</script>

<span
  class="text-[10px] font-medium px-2.5 py-[2px] rounded-full border"
  style="
    color: {STATUS_COLOR[state]};
    border-color: {STATUS_COLOR[state]}30;
    background: {STATUS_COLOR[state]}15;
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    box-shadow: 0 0 8px {STATUS_COLOR[state]}20, inset 0 0.5px 0 rgba(255,255,255,0.06);
  "
>
  {STATUS_LABEL[state]}
</span>
