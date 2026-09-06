<script lang="ts">
    import UserIdentity from "./UserIdentity.svelte";
    import { goto } from "$app/navigation";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    import { userFromPubkeyOrNpub } from "$lib/utils/nostr";
    let { id, onSaved }: { id: string; onSaved?: () => void | Promise<void> } =
        $props();
    const api = new KeycastApi();
    let pubkey = $state("");
    const memberPreview = $derived(userFromPubkeyOrNpub(pubkey));
    let role: "admin" | "member" = $state("member");
    let error = $state("");
    let busy = $state(false);
    async function submit() {
        const user = getCurrentUser()?.user;
        if (!user || busy) return;
        const member = userFromPubkeyOrNpub(pubkey.trim());
        if (!member) {
            error = "Enter a valid public key or npub.";
            return;
        }
        error = "";
        busy = true;
        try {
            const endpoint = `/teams/${id}/users`;
            const request = { user_public_key: member.pubkey, role };
            const authorization = await api.buildAuthHeader(
                endpoint,
                "POST",
                user.pubkey,
                JSON.stringify(request),
            );
            await api.post(endpoint, request, {
                headers: { Authorization: authorization },
            });
            if (onSaved) await onSaved();
            else await goto(`/teams/${id}`);
        } catch (e) {
            error = e instanceof Error ? e.message : "Could not add member";
        } finally {
            busy = false;
        }
    }
</script>

<h2 class="page-header">Add a member</h2>
<form
    onsubmit={(event) => {
        event.preventDefault();
        void submit();
    }}
>
    <div class="form-group">
        <label for="member-pubkey">Public key or npub</label><input
            id="member-pubkey"
            type="text"
            required
            bind:value={pubkey}
            placeholder="npub1…"
        />
    </div>
    {#if memberPreview}<div class="mb-4">
            <UserIdentity pubkey={memberPreview.pubkey} />
        </div>{/if}
    <div class="form-group">
        <label for="member-role">Role</label><select
            id="member-role"
            bind:value={role}
            ><option value="member">Member</option><option value="admin"
                >Admin</option
            ></select
        >
        <p class="description">
            Admins can manage keys, policies, grants, and team members.
        </p>
    </div>
    {#if error}<p role="alert" class="input-error">{error}</p>{/if}
    <button class="button button-primary" disabled={busy}
        >{busy ? "Awaiting approval…" : "Add member"}</button
    >
</form>
