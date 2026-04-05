<script lang="ts">
  import type { AgentCli } from '$lib/types';
  import { CLI_COLOR, CLI_ICON_LABEL } from '$lib/types';

  let {
    cli,
    size = 14,
  }: { cli: AgentCli; size?: number } = $props();

  let color = $derived(CLI_COLOR[cli] || '#888');
  let label = $derived(CLI_ICON_LABEL[cli] || 'Agent');
</script>

{#if cli === 'claude-code'}
  <!-- Claude: 6-pointed starburst/asterisk -->
  <svg width={size} height={size} viewBox="0 0 14 14" fill="none" aria-label={label}>
    <g stroke={color} stroke-width="1.8" stroke-linecap="round">
      <line x1="7" y1="1.2" x2="7" y2="4.8" />
      <line x1="7" y1="9.2" x2="7" y2="12.8" />
      <line x1="2" y1="4.1" x2="5.1" y2="5.9" />
      <line x1="8.9" y1="8.1" x2="12" y2="9.9" />
      <line x1="2" y1="9.9" x2="5.1" y2="8.1" />
      <line x1="8.9" y1="5.9" x2="12" y2="4.1" />
    </g>
  </svg>
{:else if cli === 'codex'}
  <!-- Codex/OpenAI: hexagon -->
  <svg width={size} height={size} viewBox="0 0 14 14" fill="none" aria-label={label}>
    <polygon
      points="7,1.2 11.8,3.6 11.8,10.4 7,12.8 2.2,10.4 2.2,3.6"
      fill="none"
      stroke={color}
      stroke-width="1.4"
      stroke-linejoin="round"
    />
  </svg>
{:else if cli === 'gemini'}
  <!-- Gemini: 4-pointed pinched star -->
  <svg width={size} height={size} viewBox="0 0 14 14" fill="none" aria-label={label}>
    <path
      d="M7 0.8 C7.2 4.5, 4.5 7.2, 0.8 7 C4.5 6.8, 6.8 9.5, 7 13.2 C6.8 9.5, 9.5 6.8, 13.2 7 C9.5 7.2, 7.2 4.5, 7 0.8Z"
      fill={color}
    />
  </svg>
{:else}
  <!-- Unknown: simple circle -->
  <svg width={size} height={size} viewBox="0 0 14 14" fill="none" aria-label={label}>
    <circle cx="7" cy="7" r="5" stroke="#888" stroke-width="1.4" fill="none" />
  </svg>
{/if}
