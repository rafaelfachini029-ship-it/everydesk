<!--
  Icon — thin-stroke lucide icon.
  Props:
    icon: a lucide-svelte component (e.g. `import { Monitor } from 'lucide-svelte'`)
    size?: number = 20 (px)
    label?: string — when set the icon is announced (role="img"); otherwise it is decorative
-->
<script lang="ts">
  import type { ComponentType, SvelteComponent } from 'svelte'
  import type { IconProps } from 'lucide-svelte'

  interface Props {
    icon: ComponentType<SvelteComponent<IconProps>>
    size?: number
    label?: string
  }
  let { icon: Glyph, size = 20, label }: Props = $props()
</script>

<span class="icon" role={label ? 'img' : undefined} aria-label={label} aria-hidden={label ? undefined : 'true'}>
  <Glyph {size} strokeWidth={1.5} absoluteStrokeWidth />
</span>

<style>
  .icon {
    display: inline-flex;
    flex: none;
    line-height: 0;
    color: currentColor;
  }
</style>
