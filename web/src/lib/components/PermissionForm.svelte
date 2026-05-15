<script lang="ts">
import {
    AVAILABLE_PERMISSIONS,
    type AllowedKindsConfig,
    type ContentFilterConfig,
} from "$lib/types";
import {
    parseAllowedKindsInput,
    parseBlockedWordsInput,
} from "$lib/utils/permission_config";
import { toTitleCase } from "$lib/utils/strings";
import Tooltip from "./Tooltip.svelte";

let { identifier = $bindable(), config = $bindable() } = $props();

let allowedKinds: string = $state("");

let contentFilterWords: string = $state("");

let allowedKindsConfig: AllowedKindsConfig = $state({
    allowed_kinds: null,
});

$effect(() => {
    allowedKindsConfig.allowed_kinds = parseAllowedKindsInput(allowedKinds);

    contentFilterConfig.blocked_words = parseBlockedWordsInput(contentFilterWords);

    switch (identifier) {
        case "allowed_kinds":
            config = allowedKindsConfig;
            break;
        case "content_filter":
            config = contentFilterConfig;
            break;
    }
});

let contentFilterConfig: ContentFilterConfig = $state({
    blocked_words: null,
});
</script>

<div class="flex flex-col items-start w-full">
    <div class="form-group w-full">
        <label for="permission">Permission Type</label>
        <select bind:value={identifier} class="w-full">
            <option value={null}>Select a permission type...</option>
            {#each AVAILABLE_PERMISSIONS as permission_identifier}
                <option value={permission_identifier}>{toTitleCase(permission_identifier)}</option>
            {/each}
        </select>
    </div>
    {#if identifier === "allowed_kinds"}
        <h3 class="flex flex-row items-center gap-2 mt-6 mb-1 font-semibold text-gray-300">
            Which event kinds are allowed
            <Tooltip content="Enter the allowed kinds as a comma separated list. Blank allows all event kinds. e.g. 1, 7, 10002." size={18} />
        </h3>
        <a href="https://github.com/nostr-protocol/nips?tab=readme-ov-file#event-kinds" target="_blank" rel="noreferrer" class="text-xs text-gray-400 border-b border-gray-400 border-dashed hover:border-solid mb-4 inline-block">List of event kinds</a>
        <div class="flex flex-col gap-2 w-full">
            <div class="grid grid-cols-[auto_1fr] items-center gap-2 w-full">
                <label class="text-base font-medium my-0! py-0!" for="allowedKinds">Kinds</label>
                <input id="allowedKinds" class="w-full" type="text" bind:value={allowedKinds} />
            </div>
        </div>
    {:else if identifier === "content_filter"}
        <h3 class="flex flex-row items-center gap-2 mt-6 mb-1 font-semibold text-gray-300">
            List of blocked words
            <Tooltip content="Comma separated list. If any of these words are present in the content of the message signing and encrypting will be refused." size={18} />
        </h3>
        <div class="flex flex-col gap-2 w-full">
            <div class="grid grid-cols-[auto_1fr] items-center gap-2 w-full">
                <label class="text-base font-medium my-0! py-0!" for="contentFilterBlockedWords">Blocked words</label>
                <input class="w-full" type="text" bind:value={contentFilterWords} />
            </div>
        </div>
    {:else if identifier === "encrypt_to_self"}
        <span class="mt-6">This permission allows for encryption/decryption but only when encrypting event to the same pubkey as the event pubkey field.</span>
    {/if}
</div>
