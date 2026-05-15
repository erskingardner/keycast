<script lang="ts"> 
import type { Instance } from "tippy.js";
import 'tippy.js/dist/tippy.css';
import { Info } from "phosphor-svelte";

let { content = $bindable(), size = 20 }: { content: string, size: number } = $props();

let tooltip: Element | null = $state(null);
let instance: Instance | null = null;

$effect(() => {
    if (!tooltip) return;

    let cancelled = false;

    void import("tippy.js").then(({ default: tippy }) => {
        if (cancelled || !tooltip) return;

        instance?.destroy();
        instance = tippy(tooltip, { content });
    });

    return () => {
        cancelled = true;
        instance?.destroy();
        instance = null;
    };
});
</script>

<span bind:this={tooltip}>
    <Info {size} />
</span>
