<script lang="ts">
  import type { Status } from '$lib/types';
  import { STATUS_LABEL, STATUS_COLOR } from '$lib/types';

  let { state }: { state: Status } = $props();
</script>

<span
  class="text-[10px] font-medium px-2 py-[2px] rounded-full border"
  style="
    color: {STATUS_COLOR[state]};
    border-color: {STATUS_COLOR[state]}59;
    background-color: {STATUS_COLOR[state]}1a;
  "
>
  {STATUS_LABEL[state]}
</span>
