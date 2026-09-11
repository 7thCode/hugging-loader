<script lang="ts">
  let { percent }: { percent: number } = $props()
</script>

<div class="track">
  <div class="fill" style:width="{percent}%"></div>
</div>

<style>
  .track {
    width: 100px;
    height: 6px;
    background: var(--bg-alt);
    border-radius: 3px;
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 150ms ease-out;
  }
</style>
