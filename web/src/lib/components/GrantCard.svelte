<script lang="ts">
import type { Grant } from "$lib/types";
import { formattedUnixDateTime } from "$lib/utils/dates";
import { LinkSimple, Trash } from "phosphor-svelte";

let {
    grant,
    onRevoke,
    onCreateInvitation,
    onRevokeInvitation,
}: {
    grant: Grant;
    onRevokeInvitation: (grant: Grant, invitationId: number) => Promise<void>;
    onRevoke: (grant: Grant) => Promise<void> | void;
    onCreateInvitation: (grant: Grant) => Promise<void> | void;
} = $props();

let busy = $state(false);

async function run(action: (grant: Grant) => Promise<void> | void) {
    busy = true;
    try {
        await action(grant);
    } finally {
        busy = false;
    }
}
</script>

<div class="card">
    <div>
        <h3 class="text-lg font-semibold">{grant.name}</h3>
        <p class="font-mono text-xs text-gray-500 break-all">{grant.remote_signer_public_key}</p>
    </div>
    <div class="grid grid-cols-[auto_1fr] gap-y-1 gap-x-2 text-xs text-gray-400">
        <span>Sessions:</span><span>{grant.active_sessions}</span>
        <span>Open invitations:</span><span>{grant.claimable_invitations}</span>
        <span>Policy:</span><span>#{grant.policy_id}</span>
        <span>Expiration:</span><span>{formattedUnixDateTime(grant.expires_at)}</span>
        <span>Status:</span><span>{grant.revoked_at ? "Revoked" : "Active"}</span>
    </div>
    {#each grant.invitations as invitation}
        <div class="text-xs text-gray-400">
            Invitation #{invitation.id} · expires {formattedUnixDateTime(invitation.expires_at)}
            <button type="button" class="button button-secondary" disabled={busy} onclick={() => run(g => onRevokeInvitation(g, invitation.id))}>Revoke invitation</button>
        </div>
    {/each}
    {#if !grant.revoked_at}
        <button type="button" onclick={() => run(onCreateInvitation)} class="button button-secondary button-icon" disabled={busy}>
            <LinkSimple size="20" /> New one-time invitation
        </button>
        <button type="button" onclick={() => run(onRevoke)} class="button button-danger button-icon" disabled={busy}>
            <Trash size="20" /> Revoke grant
        </button>
    {/if}
</div>
