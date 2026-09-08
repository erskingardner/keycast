<script lang="ts">
    import { goto } from "$app/navigation";
    import GrantEditor from "$lib/components/GrantEditor.svelte";
    import Copy from "$lib/components/Copy.svelte";
    import PublicKeyDetails from "$lib/components/PublicKeyDetails.svelte";
    import GrantCard from "$lib/components/GrantCard.svelte";
    import Loader from "$lib/components/Loader.svelte";
    import { getCurrentUser } from "$lib/current_user.svelte";
    import { KeycastApi } from "$lib/keycast_api.svelte";
    import type {
        Grant,
        InvitationCreationResponse,
        KeyWithRelations,
        KeyRelayInfo,
        StoredKey,
        Team,
    } from "$lib/types";
    import { toast } from "svelte-hot-french-toast";

    let {
        id,
        pubkey,
        onRemoved,
    }: { id: string; pubkey: string; onRemoved?: () => void | Promise<void> } =
        $props();
    let addingGrant = $state(false);
    let started = $state(false);
    const api = new KeycastApi();
    const user = $derived(getCurrentUser()?.user);
    let isLoading = $state(true);
    let team: Team | null = $state(null);
    let key: StoredKey | null = $state(null);
    let grants: Grant[] = $state([]);
    let relayDiscovery: KeyRelayInfo | null = $state(null);
    let invitationUri: string | null = $state(null);
    let loadError: string | null = $state(null);

    $effect(() => {
        if (!user?.pubkey || started) return;
        started = true;
        void load();
    });

    async function load() {
        if (!user?.pubkey) return;
        isLoading = true;
        loadError = null;
        const endpoint = `/teams/${id}/keys/${pubkey}`;
        await api
            .buildAuthHeader(endpoint, "GET", user.pubkey)
            .then((authorization) =>
                api.get<KeyWithRelations>(endpoint, {
                    headers: { Authorization: authorization },
                }),
            )
            .then((response) => {
                key = response.stored_key;
                team = response.team;
                grants = response.grants;
                relayDiscovery = response.relay_discovery;
            })
            .catch((error) => {
                loadError =
                    error instanceof Error ? error.message : String(error);
            })
            .finally(() => {
                isLoading = false;
            });
    }

    async function removeKey() {
        if (
            !user?.pubkey ||
            !confirm(
                "Remove this key and revoke every grant and session attached to it?",
            )
        )
            return;
        try {
            const endpoint = `/teams/${id}/keys/${pubkey}`;
            const authorization = await api.buildAuthHeader(
                endpoint,
                "DELETE",
                user.pubkey,
            );
            await api.delete(endpoint, {
                headers: { Authorization: authorization },
            });
            toast.success("Key removed");
            if (onRemoved) await onRemoved();
            else await goto(`/teams/${id}`);
        } catch (error) {
            toast.error(error instanceof Error ? error.message : String(error));
        }
    }

    async function revokeGrant(grant: Grant) {
        if (
            !user?.pubkey ||
            !confirm("Revoke this grant and all of its active sessions?")
        )
            return;
        try {
            const endpoint = `/teams/${id}/keys/${pubkey}/grants/${grant.id}`;
            const authorization = await api.buildAuthHeader(
                endpoint,
                "DELETE",
                user.pubkey,
            );
            await api.delete(endpoint, {
                headers: { Authorization: authorization },
            });
            grants = grants.map((item) =>
                item.id === grant.id
                    ? {
                          ...item,
                          revoked_at: Math.floor(Date.now() / 1000),
                          active_sessions: 0,
                          claimable_invitations: 0,
                          invitations: [],
                      }
                    : item,
            );
            toast.success("Grant revoked");
        } catch (error) {
            toast.error(error instanceof Error ? error.message : String(error));
        }
    }

    async function createInvitation(grant: Grant) {
        if (!user?.pubkey) return;
        try {
            const endpoint = `/teams/${id}/grants/${grant.id}/invitations`;
            const request = {
                expires_at: Math.min(
                    Math.floor(Date.now() / 1000) + 24 * 3600,
                    grant.expires_at ?? Infinity,
                ),
            };
            const body = JSON.stringify(request);
            const authorization = await api.buildAuthHeader(
                endpoint,
                "POST",
                user.pubkey,
                body,
            );
            const response = await api.post<InvitationCreationResponse>(
                endpoint,
                request,
                { headers: { Authorization: authorization } },
            );
            invitationUri = response.bunker_uri;
            grants = grants.map((item) =>
                item.id === grant.id
                    ? {
                          ...item,
                          claimable_invitations: item.claimable_invitations + 1,
                          invitations: [
                              ...item.invitations,
                              {
                                  id: response.invitation_id,
                                  expires_at: request.expires_at,
                              },
                          ],
                      }
                    : item,
            );
            toast.success("One-time invitation created");
        } catch (error) {
            toast.error(error instanceof Error ? error.message : String(error));
        }
    }
    async function revokeInvitation(grant: Grant, invitationId: number) {
        if (!user?.pubkey) return;
        try {
            const endpoint = `/teams/${id}/invitations/${invitationId}`;
            const authorization = await api.buildAuthHeader(
                endpoint,
                "DELETE",
                user.pubkey,
            );
            await api.delete(endpoint, {
                headers: { Authorization: authorization },
            });
            grants = grants.map((item) =>
                item.id === grant.id
                    ? {
                          ...item,
                          claimable_invitations: Math.max(
                              0,
                              item.claimable_invitations - 1,
                          ),
                          invitations: item.invitations.filter(
                              (i) => i.id !== invitationId,
                          ),
                      }
                    : item,
            );
            invitationUri = null;
            toast.success("Invitation revoked; active sessions are unchanged");
        } catch (error) {
            toast.error(error instanceof Error ? error.message : String(error));
        }
    }
</script>

{#if isLoading}
    <Loader />
{:else if loadError}
    <div role="alert">
        <p class="input-error">{loadError}</p>
        <button class="button button-secondary mt-3" onclick={load}
            >Retry</button
        >
    </div>
{:else if team && key}
    <div class="flex flex-wrap items-center justify-between gap-3 mb-5">
        <PublicKeyDetails pubkey={key.public_key} />
        <button class="button button-danger" onclick={removeKey}
            >Remove key</button
        >
    </div>
    {#if relayDiscovery}
        <section class="border-t border-border py-4 mb-4">
            <div class="flex items-center justify-between gap-3 mb-2">
                <h3 class="font-mono text-sm">Relays from this key</h3>
                <button class="button button-secondary" onclick={load}>Refresh</button>
            </div>
            <p class="text-muted text-sm mb-3">
                NIP-65 · {relayDiscovery.status}
                {#if relayDiscovery.fetched_at} · checked {new Date(relayDiscovery.fetched_at * 1000).toLocaleString()}{/if}
            </p>
            {#each relayDiscovery.relays as relay (relay.url)}
                <div class="flex flex-wrap justify-between gap-2 border-t border-border py-2 text-sm">
                    <span class="font-mono break-all">{relay.url}</span>
                    <span class="text-muted">{relay.read ? "read" : ""}{relay.read && relay.write ? " / " : ""}{relay.write ? "write" : ""} · {relay.active ? "signer active" : relay.status}{!relay.listed ? " · retiring" : ""}</span>
                </div>
            {:else}
                <p class="text-muted text-sm">{relayDiscovery.status === "pending" ? "Looking for this key’s relay list in the background." : "No usable relay list found. Your configured signing relays remain available."}</p>
            {/each}
            {#if !relayDiscovery.auto_activate}
                <p class="text-muted text-sm mt-3">Automatic activation is off. An instance operator can enable it in Instance status.</p>
            {/if}
        </section>
    {/if}
    {#if invitationUri}
        <div class="invitation">
            <h3 class="text-sm font-semibold mb-2">Copy this invitation now</h3>
            <p class="description mb-3">Its secret cannot be shown again.</p>
            <div class="font-mono text-xs break-all flex items-center gap-2">
                <span class="grow">{invitationUri}</span><Copy
                    value={invitationUri}
                />
            </div>
            <button
                class="button button-secondary mt-3"
                onclick={() => (invitationUri = null)}>Dismiss</button
            >
        </div>
    {/if}
    <div class="section-heading">
        <h2>App access <span>{grants.length}</span></h2>
        <button
            class="button button-secondary"
            aria-expanded={addingGrant}
            onclick={() => (addingGrant = !addingGrant)}
            >{addingGrant ? "Close form" : "Connect app"}</button
        >
    </div>
    {#if addingGrant}
        <div class="form-panel">
            <GrantEditor
                {id}
                {pubkey}
                onDone={async () => {
                    addingGrant = false;
                    await load();
                }}
            />
        </div>
    {/if}
    {#if grants.length === 0}
        <div class="empty-state">
            No apps connected. Create a grant to choose a policy and generate a
            one-time invitation.
        </div>
    {:else}
        <div class="flex flex-col gap-2">
            {#each grants as grant (grant.id)}
                <GrantCard
                    {grant}
                    onRevoke={revokeGrant}
                    onCreateInvitation={createInvitation}
                    onRevokeInvitation={revokeInvitation}
                />
            {/each}
        </div>
    {/if}
{/if}
