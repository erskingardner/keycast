<script lang="ts">
    import { Check, Copy } from "phosphor-svelte";
    import { toast } from "svelte-hot-french-toast";
    let {
        value,
        size = "16",
        showText = false,
        extraClasses = "",
        label = "Copy",
    }: {
        value: string;
        size?: string;
        showText?: boolean;
        extraClasses?: string;
        label?: string;
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
    aria-label={copied ? "Copied" : `${label} to clipboard`}
    title={copied ? "Copied" : label}
    class="inline-flex items-center gap-1 p-1 shrink-0 {extraClasses}"
>
    {#if copied}<Check weight="bold" {size} class="text-accent" />{:else}<Copy
            {size}
        />{/if}
    {#if showText}{copied ? "Copied" : label}{/if}
</button>
