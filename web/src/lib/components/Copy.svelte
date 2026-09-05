<script lang="ts">
    import { Check, Copy } from "phosphor-svelte";
    import { toast } from "svelte-hot-french-toast";
    let {
        value,
        size = "16",
        showText = false,
        extraClasses = "",
    }: {
        value: string;
        size?: string;
        showText?: boolean;
        extraClasses?: string;
    } = $props();
    let copied = $state(false);
    async function copy() {
        try {
            await navigator.clipboard.writeText(value);
            copied = true;
            setTimeout(() => (copied = false), 1500);
        } catch {
            toast.error("Could not copy. Select and copy the value manually.");
        }
    }
</script>

<button
    type="button"
    onclick={copy}
    aria-label={copied ? "Copied" : "Copy to clipboard"}
    class="inline-flex items-center gap-1 p-1 shrink-0 {extraClasses}"
>
    {#if copied}<Check weight="bold" {size} class="text-accent" />{:else}<Copy
            {size}
        />{/if}
    {#if showText}{copied ? "Copied" : "Copy"}{/if}
</button>
