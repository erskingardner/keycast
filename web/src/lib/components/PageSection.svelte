<script lang="ts">
    import type { Snippet } from "svelte";
    let { title, children }: { title: string; children: Snippet } = $props();
</script>

<section class="section">
    <div class="section-heading"><h2>{title}</h2></div>
    {@render children()}
</section>
