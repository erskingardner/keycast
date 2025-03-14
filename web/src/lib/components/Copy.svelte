<script lang="ts">
import { Check, Copy } from "phosphor-svelte";

let {
    value,
    size = "20",
    showText = false,
    extraClasses,
}: {
    value: string;
    size?: string;
    showText?: boolean;
    extraClasses?: string;
} = $props();

let copySuccess = $state(false);

async function copyListId() {
    copyToClipboard(value).then(() => {
        copySuccess = true;
        setTimeout(() => {
            copySuccess = false;
        }, 1500);
    });
}

async function copyToClipboard(textToCopy: string) {
    try {
        await navigator.clipboard.writeText(textToCopy);
    } catch (err) {
        console.error("Failed to copy: ", err);
    }
}
</script>
    
    <button onclick={copyListId} class="border-none outline-hidden ring-none {extraClasses}">
        {#if copySuccess}
            <Check weight="light" {size} class="text-green-500" />
        {:else}
            <Copy weight="light" {size} />
        {/if}
        {#if showText}
            Copy ID
        {/if}
    </button>
    