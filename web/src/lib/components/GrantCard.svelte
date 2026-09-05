<script lang="ts">
    import type { Grant } from "$lib/types";
    import { formattedUnixDateTime } from "$lib/utils/dates";
    import { LinkSimple } from "phosphor-svelte";

    let {
        grant,
        onRevoke,
        onCreateInvitation,
        onRevokeInvitation,
    }: {
        grant: Grant;
        onRevokeInvitation: (
            grant: Grant,
            invitationId: number,
        ) => Promise<void>;
        onRevoke: (grant: Grant) => Promise<void> | void;
        onCreateInvitation: (grant: Grant) => Promise<void> | void;
    } = $props();

    let busy = $state(false);
    const expired = $derived(
        grant.expires_at !== null &&
            grant.expires_at <= Math.floor(Date.now() / 1000),
    );

    async function run(action: (grant: Grant) => Promise<void> | void) {
        busy = true;
        try {
            await action(grant);
        } finally {
            busy = false;
        }
    }
</script>

<div class="grant-row">
    <div class="flex justify-between items-center gap-3">
        <h3 class="text-sm font-semibold">{grant.name}</h3>
        <span class="badge" class:badge-neutral={!!grant.revoked_at || expired}
            >{grant.revoked_at
                ? "Revoked"
                : expired
                  ? "Expired"
                  : "Active"}</span
        >
    </div>
    <div class="grant-meta">
        <span
            >{grant.active_sessions}
            {grant.active_sessions === 1 ? "session" : "sessions"}</span
        ><span
            >{grant.claimable_invitations}
            {grant.claimable_invitations === 1
                ? "invitation"
                : "invitations"}</span
        ><span>Policy #{grant.policy_id}</span><span
            >{grant.expires_at
                ? `Expires ${formattedUnixDateTime(grant.expires_at)}`
                : "No expiration"}</span
        >
    </div>
    <details class="text-xs text-muted mb-3">
        <summary>Connection identity</summary>
        <p class="font-mono break-all mt-2">{grant.remote_signer_public_key}</p>
    </details>
    {#each grant.invitations as invitation}
        <div
            class="flex flex-wrap items-center justify-between gap-2 py-2 border-t border-line text-xs text-muted"
        >
            <span
                >Invitation #{invitation.id} · expires {formattedUnixDateTime(
                    invitation.expires_at,
                )}</span
            >
            <button
                type="button"
                class="button button-quiet"
                disabled={busy}
                onclick={() => run((g) => onRevokeInvitation(g, invitation.id))}
                >Revoke invitation</button
            >
        </div>
    {/each}
    {#if !grant.revoked_at}
        <div class="flex flex-wrap gap-2">
            <button
                type="button"
                onclick={() => run(onCreateInvitation)}
                class="button button-secondary"
                disabled={busy || expired}
                ><LinkSimple size={15} /> New invitation</button
            >
            <button
                type="button"
                onclick={() => run(onRevoke)}
                class="button button-danger"
                disabled={busy}>Revoke access</button
            >
        </div>
    {/if}
</div>
