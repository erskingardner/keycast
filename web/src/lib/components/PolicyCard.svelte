<script lang="ts">
    import type { Policy } from "$lib/types";
    import { PencilSimple, Trash } from "phosphor-svelte";

    let {
        policy,
        hoverable = false,
        editHref,
        onRemove,
        onEdit,
    }: {
        policy: Policy;
        hoverable?: boolean;
        editHref?: string;
        onEdit?: (policy: Policy) => void;
        onRemove?: (policy: Policy) => Promise<void> | void;
    } = $props();

    let busy = $state(false);

    async function remove() {
        if (!onRemove) return;
        busy = true;
        try {
            await onRemove(policy);
        } finally {
            busy = false;
        }
    }

    const descriptions = $derived.by(() => {
        const capabilities = policy.document?.capabilities;
        if (
            !capabilities ||
            typeof capabilities !== "object" ||
            Array.isArray(capabilities)
        ) {
            return ["Unknown policy document; all requests are denied."];
        }
        const values: string[] = [];
        if (capabilities.sign_event) {
            values.push(
                `Sign event kinds: ${(capabilities.sign_event.allowed_kinds ?? []).join(", ")}`,
            );
        }
        for (const method of [
            "nip04_encrypt",
            "nip04_decrypt",
            "nip44_encrypt",
            "nip44_decrypt",
        ] as const) {
            const capability = capabilities[method];
            if (capability) {
                values.push(
                    `${method}: ${capability.recipient === "any" ? "any counterparty" : "self only"}`,
                );
            }
        }
        return values;
    });
</script>

<div class={hoverable ? "hover-card" : ""}>
    <div class="flex items-baseline justify-between gap-2">
        <h3 class="text-sm font-semibold">{policy.name}</h3>
        <span class="text-xs text-muted">rev {policy.revision}</span>
    </div>
    <ul class="mt-2 space-y-1">
        {#each descriptions as description}
            <li class="text-xs text-muted">{description}</li>
        {/each}
    </ul>
    {#if editHref || onEdit || onRemove}
        <div class="flex flex-wrap gap-2 mt-2">
            {#if onEdit}<button
                    type="button"
                    class="button button-secondary"
                    onclick={() => onEdit?.(policy)}>Edit</button
                >{/if}
            {#if editHref}<a
                    href={editHref}
                    class="button button-secondary button-icon"
                    ><PencilSimple size="18" /> Edit</a
                >{/if}
            {#if onRemove}<button
                    type="button"
                    class="button button-danger button-icon"
                    onclick={remove}
                    disabled={busy}><Trash size="18" /> Delete</button
                >{/if}
        </div>
    {/if}
</div>
