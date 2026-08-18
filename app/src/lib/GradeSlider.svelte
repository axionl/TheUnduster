<script lang="ts">
  // Labeled range slider for the grade panel. `value` is bindable so callers
  // can two-way bind straight into a GradeSettings field (the shared $state
  // object App owns), keeping every slider's markup to one line.
  let {
    label,
    value = $bindable(),
    min,
    max,
    step,
    format,
  }: {
    label: string;
    value: number;
    min: number;
    max: number;
    step: number;
    format?: (value: number) => string;
  } = $props();
</script>

<label class="grow">
  <span class="grow-label">{label}</span>
  <input type="range" {min} {max} {step} bind:value aria-label={label} />
  <span class="grow-value">{format ? format(value) : value.toFixed(2)}</span>
</label>

<style>
  .grow {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
  }
  .grow-label {
    min-width: 8ch;
    color: var(--text-1);
  }
  .grow input[type="range"] {
    flex: 1;
    min-width: 0;
  }
  .grow-value {
    min-width: 5ch;
    text-align: right;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }
</style>
