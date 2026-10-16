<script lang="ts">
  // Policy selector: presets, a stored custom period, and "Outro…" (a whole number of minutes,
  // RECONFIRM_MIN..RECONFIRM_MAX). `onchange` gets a policy key (see policy.ts) only once the
  // choice is complete; "Outro…" waits for a valid number and OK.
  import {
    OTHER_KEY,
    RECONFIRM_MAX,
    RECONFIRM_MIN,
    isValidPeriod,
    policyFromKey,
    policyOptions,
  } from './policy'

  let { value, onchange }: { value: string; onchange: (key: string) => void } = $props()

  let other = $state(false)
  let minutes = $state<number | null>(null)

  function pick(key: string) {
    if (key === OTHER_KEY) {
      other = true
      minutes = null
      return
    }
    other = false
    onchange(key)
  }

  function confirmOther(e: Event) {
    e.preventDefault()
    if (minutes === null || !isValidPeriod(minutes)) return
    other = false
    onchange(`r${minutes}`)
  }
</script>

<select value={other ? OTHER_KEY : value} onchange={(e) => pick(e.currentTarget.value)}>
  {#each policyOptions(policyFromKey(value)) as opt (opt.key)}
    <option value={opt.key}>{opt.label}</option>
  {/each}
</select>
{#if other}
  <span class="other">
    a cada
    <input
      type="number"
      min={RECONFIRM_MIN}
      max={RECONFIRM_MAX}
      step="1"
      bind:value={minutes}
      onkeydown={(e) => e.key === 'Enter' && confirmOther(e)}
    />
    min
    <button onclick={confirmOther} disabled={minutes === null || !isValidPeriod(minutes)}>OK</button>
    <button onclick={() => (other = false)}>Cancelar</button>
  </span>
{/if}

<style>
  .other input {
    width: 5em;
  }
</style>
